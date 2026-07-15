//! The in-memory process registry: the supervisor's authoritative map of managed
//! processes plus, while one is running, the handle used to message its actor.
//!
//! The registry's `Mutex` guards only the lookup map. Each entry has exactly one
//! writer for its lifecycle state — the actor that owns it — so there is no shared
//! mutable domain state behind a lock beyond the map itself.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::SpawnSpec;
use crate::process::{ProcStatus, ProcessKind, ProcessView, Readiness};
use crate::sync::lock;

use super::actor::{ActorMsg, OrphanIdentity};

mod actors;
mod queries;

/// A live actor's control surface: a bounded mailbox to message it and — once the task is
/// spawned — the join handle awaited at shutdown to confirm its child was reaped. The mailbox
/// is installed as the launch is claimed ([`Registry::begin_launch`]) so a stop or shutdown that
/// lands in the launch window still reaches the actor; the join is attached a moment later
/// ([`Registry::attach_join`]) once the task exists, so it is `None` for that window only.
pub(crate) struct ActorHandle {
    pub(crate) mailbox: mpsc::Sender<ActorMsg>,
    pub(crate) join: Option<JoinHandle<()>>,
}

/// One managed process: its read-model view, everything needed to (re)launch it, and
/// — while running — its actor handle.
struct Managed {
    view: ProcessView,
    launch: SpawnSpec,
    /// The project root this process belongs to — part of its orphan-adoption identity.
    project_root: PathBuf,
    trust_variant: Option<Hash>,
    auto_start: bool,
    auto_restart: bool,
    /// Globs (relative to `project_root`) whose changes file-watch-restart this command.
    /// Empty for terminals, agents, and commands that declare no `restart_when_changed`.
    restart_when_changed: Vec<String>,
    /// An alternate command line that relaunches this process resuming its last session
    /// (an agent's "Resume last session"), or `None`. Stored and replayed verbatim — never
    /// interpreted; [`Supervisor::resume`](super::Supervisor::resume) launches it in place of
    /// the fresh `launch.command`, leaving that command untouched so a later plain start is
    /// still fresh.
    resume_command: Option<String>,
    handle: Option<ActorHandle>,
    /// The leader pgid of the running OS process group, while one is live. Recorded by the
    /// actor after spawn and cleared when the child is reaped, so monitoring can sample the
    /// group. `None` whenever the process is resting.
    pgid: Option<i32>,
}

/// A cloned read of one entry's launch-relevant fields, taken under the lock so the
/// supervisor can make trust and eligibility decisions without holding it.
pub(crate) struct EntryInfo {
    pub(crate) project: ProjectId,
    pub(crate) status: ProcStatus,
    pub(crate) trust_variant: Option<Hash>,
    pub(crate) auto_restart: bool,
    pub(crate) launch: SpawnSpec,
    /// The resume command line, if this process can resume its last session — replayed by
    /// [`Supervisor::resume`](super::Supervisor::resume).
    pub(crate) resume_command: Option<String>,
}

/// A start candidate: its id, trust variant (if trust-gated), and launch spec.
pub(crate) struct Candidate {
    pub(crate) id: ProcessId,
    pub(crate) trust_variant: Option<Hash>,
    pub(crate) launch: SpawnSpec,
}

/// The process registry. Cloneable; all clones share one map.
#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<HashMap<ProcessId, Managed>>>,
}

impl Registry {
    /// Records a freshly registered (stopped) process.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add(
        &self,
        view: ProcessView,
        launch: SpawnSpec,
        project_root: PathBuf,
        trust_variant: Option<Hash>,
        auto_start: bool,
        auto_restart: bool,
        restart_when_changed: Vec<String>,
        resume_command: Option<String>,
    ) {
        let mut guard = lock(&self.inner);
        guard.insert(
            view.id,
            Managed {
                view,
                launch,
                project_root,
                trust_variant,
                auto_start,
                auto_restart,
                restart_when_changed,
                resume_command,
                handle: None,
                pgid: None,
            },
        );
    }

    /// Records (or clears) the leader pgid of a process's running OS group. The actor sets
    /// it after a successful spawn and clears it (`None`) when the child is reaped, so the
    /// monitoring samplers only ever target a process with a live group. Clearing the group
    /// also clears its discovered ports — a process with no live group has none.
    pub(crate) fn set_pgid(&self, id: ProcessId, pgid: Option<i32>) {
        let mut guard = lock(&self.inner);
        if let Some(entry) = guard.get_mut(&id) {
            entry.pgid = pgid;
            if pgid.is_none() {
                // A process with no live group has neither discovered ports nor a readiness
                // gate — clear both so a resting process never shows stale monitoring state.
                entry.view.ports.clear();
                entry.view.ready = Readiness::Ungated;
            }
        }
    }

    /// Every process with a live OS group, as `(id, leader pgid)` — the monitoring samplers'
    /// targets each tick.
    pub(crate) fn live_groups(&self) -> Vec<(ProcessId, i32)> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter_map(|entry| entry.pgid.map(|pgid| (entry.view.id, pgid)))
            .collect()
    }

    /// The leader pgid of a single process's live OS group, if it has one (i.e. it is
    /// running) — used by a port-readiness wait to know which group to probe.
    pub(crate) fn pgid_of(&self, id: ProcessId) -> Option<i32> {
        lock(&self.inner).get(&id).and_then(|entry| entry.pgid)
    }

    /// The managed process whose live OS group leader is `pgid`, if any — the caller's *home*
    /// process. Each managed process is its own group leader (a unique pgid), so at most one
    /// matches. The identity check resolves a connecting peer's process group to the process
    /// it runs in, so a session can only bind to (and scope within) its own process tree.
    pub(crate) fn process_at_pgid(&self, pgid: i32) -> Option<ProcessId> {
        lock(&self.inner)
            .values()
            .find(|entry| entry.pgid == Some(pgid))
            .map(|entry| entry.view.id)
    }

    /// The display label of a process by id, `None` if it is no longer registered — what the
    /// notification reactor names a toast after.
    pub(crate) fn label_of(&self, id: ProcessId) -> Option<String> {
        lock(&self.inner)
            .get(&id)
            .map(|entry| entry.view.label.clone())
    }

    /// One process's read-model row by id, `None` if it is no longer registered. A focused
    /// read for consumers that need a single process, so they clone one view rather than the
    /// whole [`snapshot`](Self::snapshot).
    pub(crate) fn view(&self, id: ProcessId) -> Option<ProcessView> {
        lock(&self.inner).get(&id).map(|entry| entry.view.clone())
    }

    /// Renames a process's display label, returning whether it was still registered. The
    /// label is display-only: the launch spec, trust variant, and live group are untouched.
    pub(crate) fn set_label(&self, id: ProcessId, label: String) -> bool {
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id) {
            Some(entry) => {
                entry.view.label = label;
                true
            }
            None => false,
        }
    }

    /// Replaces a registered command's launch spec, trust variant, label, and schedule fields
    /// **in place**, keeping its id — the config-reload path applying a changed `solo.yml` spec
    /// without minting a fresh id (which would duplicate the command). The caller holds the
    /// trust store, so it computes and passes `requires_trust`. The live actor handle, pgid, and
    /// status are untouched: a running process keeps running on its current launch until its
    /// next (re)start picks up the new spec. Returns whether the process was still registered.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_command_spec(
        &self,
        id: ProcessId,
        label: String,
        launch: SpawnSpec,
        trust_variant: Option<Hash>,
        auto_start: bool,
        auto_restart: bool,
        restart_when_changed: Vec<String>,
        requires_trust: bool,
    ) -> bool {
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id) {
            Some(entry) => {
                entry.view.label = label;
                entry.view.requires_trust = requires_trust;
                entry.launch = launch;
                entry.trust_variant = trust_variant;
                entry.auto_start = auto_start;
                entry.auto_restart = auto_restart;
                entry.restart_when_changed = restart_when_changed;
                true
            }
            None => false,
        }
    }

    /// Removes a process **only if it is not active**, returning whether it was removed — the
    /// config-reload path dropping a command deleted from `solo.yml` without ever killing
    /// running work. A resting entry holds no live actor, so it is dropped outright; an active
    /// one is left untouched. Checking the status and removing under one lock keeps it atomic.
    pub(crate) fn remove_if_resting(&self, id: ProcessId) -> bool {
        let mut guard = lock(&self.inner);
        match guard.get(&id) {
            Some(entry) if !entry.view.status.is_active() => {
                guard.remove(&id);
                true
            }
            _ => false,
        }
    }

    /// Updates a process's readiness gate to ready/not-ready, but only while it is still on
    /// the `pgid` the wait is probing. Returns whether it changed (so the caller announces
    /// only real transitions). Guarding on the group closes the race where a process stops
    /// (or restarts onto a new group) mid-wait: a stale update lands on no live group and is
    /// dropped, never resurrecting readiness on a resting process.
    pub(crate) fn set_ready(&self, id: ProcessId, pgid: i32, ready: bool) -> bool {
        let next = if ready {
            Readiness::Ready
        } else {
            Readiness::Waiting
        };
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id) {
            Some(entry) if entry.pgid == Some(pgid) && entry.view.ready != next => {
                entry.view.ready = next;
                true
            }
            _ => false,
        }
    }

    /// Updates a process's discovered listening ports, but only while it is still on the
    /// `pgid` that was scanned. Returns whether the set actually changed (so the port
    /// scanner only announces real changes). Guarding on the group closes the race where a
    /// process stops (clearing its ports) or restarts mid-scan: the stale reading lands on
    /// no live group and is dropped, never resurrecting ports on a resting process. The
    /// ports are stored sorted by the caller.
    pub(crate) fn set_ports(&self, id: ProcessId, pgid: i32, ports: Vec<u16>) -> bool {
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id) {
            Some(entry) if entry.pgid == Some(pgid) && entry.view.ports != ports => {
                entry.view.ports = ports;
                true
            }
            _ => false,
        }
    }

    /// The orphan-adoption identity of `id`: its project root and name.
    pub(crate) fn identity(&self, id: ProcessId) -> Option<OrphanIdentity> {
        let guard = lock(&self.inner);
        let entry = guard.get(&id)?;
        Some(OrphanIdentity {
            project_root: entry.project_root.clone(),
            name: entry.view.label.clone(),
        })
    }

    /// A registered, resting (`Stopped`) process whose orphan identity — project root,
    /// name, and command — matches a leftover group, i.e. the process to re-attach it
    /// to. Returns `None` if nothing matches (the leftover is surfaced instead).
    pub(crate) fn find_resting_match(
        &self,
        project_root: &Path,
        name: &str,
        command: &str,
    ) -> Option<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .find(|entry| {
                entry.view.status == ProcStatus::Stopped
                    && entry.project_root == project_root
                    && entry.view.label == name
                    && entry.launch.command == command
            })
            .map(|entry| entry.view.id)
    }

    /// The launch-relevant fields for `id`, if present.
    pub(crate) fn describe(&self, id: ProcessId) -> Option<EntryInfo> {
        let guard = lock(&self.inner);
        let entry = guard.get(&id)?;
        Some(EntryInfo {
            project: entry.view.project,
            status: entry.view.status,
            trust_variant: entry.trust_variant,
            auto_restart: entry.auto_restart,
            launch: entry.launch.clone(),
            resume_command: entry.resume_command.clone(),
        })
    }

    /// The current cached status for `id`, if present.
    pub(crate) fn status(&self, id: ProcessId) -> Option<ProcStatus> {
        lock(&self.inner).get(&id).map(|entry| entry.view.status)
    }

    /// Clears the trust-required flag on every entry whose variant matches within
    /// `project` — used when the user trusts a command so the next snapshot reflects it.
    pub(crate) fn mark_variant_trusted(&self, project: ProjectId, variant: &Hash) {
        let mut guard = lock(&self.inner);
        for entry in guard.values_mut() {
            if entry.view.project == project && entry.trust_variant.as_ref() == Some(variant) {
                entry.view.requires_trust = false;
            }
        }
    }

    /// Updates the cached status and exit code for `id` (no-op once removed).
    pub(crate) fn set_status(&self, id: ProcessId, status: ProcStatus, exit_code: Option<i32>) {
        let mut guard = lock(&self.inner);
        if let Some(entry) = guard.get_mut(&id) {
            entry.view.status = status;
            entry.view.exit_code = exit_code;
        }
    }

    /// Every `Command` that declares `restart_when_changed` globs, as `(id, project_root,
    /// globs)` — the file-watch reactor's watch and match inputs. Terminals, agents, and
    /// commands with no globs are omitted (they are never file-watched). Trust is not checked
    /// here; the reactor's restart re-checks it (fail-closed), as the crash policy does.
    pub(crate) fn watch_commands(&self) -> Vec<(ProcessId, PathBuf, Vec<String>)> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.kind == ProcessKind::Command && !entry.restart_when_changed.is_empty()
            })
            .map(|entry| {
                (
                    entry.view.id,
                    entry.project_root.clone(),
                    entry.restart_when_changed.clone(),
                )
            })
            .collect()
    }

    /// A cloned snapshot of every process view — the read model adapters render.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        lock(&self.inner)
            .values()
            .map(|entry| entry.view.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ProjectId;
    use crate::ports::PtySize;
    use std::collections::BTreeMap;

    fn registry_holding(status: ProcStatus) -> (Registry, ProcessId) {
        let registry = Registry::default();
        let id = ProcessId::next();
        let view = ProcessView {
            id,
            project: ProjectId::from_raw(1),
            kind: ProcessKind::Command,
            label: "x".into(),
            status: ProcStatus::Stopped,
            exit_code: None,
            requires_trust: false,
            resumable: false,
            ports: Vec::new(),
            ready: Readiness::Ungated,
        };
        let launch = SpawnSpec {
            command: "x".into(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            size: PtySize::default(),
        };
        registry.add(
            view,
            launch,
            PathBuf::from("/"),
            None,
            false,
            false,
            Vec::new(),
            None,
        );
        registry.set_status(id, status, None);
        (registry, id)
    }

    #[tokio::test]
    async fn begin_launch_installs_the_mailbox_before_the_join_is_attached() {
        // The launch window: a stop or shutdown that lands after the launch is claimed but before
        // the actor task is spawned must still reach the actor. `begin_launch` installs the mailbox
        // under the claim lock; the join is only attached once the task exists. So `signal` reaches
        // the actor in the window, while `take_handle` (which must be able to await a reap) declines
        // until the join is present — leaving the entry for an in-place stop and a retry rather than
        // a handle it cannot await.
        let (registry, id) = registry_holding(ProcStatus::Stopped);
        let (mailbox, mut inbox) = mpsc::channel(4);

        assert_eq!(
            registry.begin_launch(id, mailbox),
            Some(ProcStatus::Stopped)
        );
        assert_eq!(registry.status(id), Some(ProcStatus::Starting));
        assert!(
            registry.take_handle(id).is_none(),
            "no join yet — left intact for an in-place stop and a retry"
        );

        registry.signal(id, ActorMsg::Stop);
        assert!(
            matches!(inbox.try_recv(), Ok(ActorMsg::Stop)),
            "an in-place signal reaches the mid-launch actor in the window"
        );

        // Once the task is spawned and its join attached, shutdown can take and await it.
        registry.attach_join(id, tokio::spawn(async {}));
        assert!(
            registry.take_handle(id).is_some(),
            "a fully attached handle is taken for reaping"
        );
    }

    #[tokio::test]
    async fn a_superseded_launch_aborts_its_orphaned_actor() {
        // If the process is closed/removed in the launch window, its handle is gone by the time
        // the task spawns: `attach_join` must abort the orphaned task rather than leave it to
        // spawn a child no one owns. Verified by the task's captured marker being dropped (the
        // future cancelled), which never happens while a `pending` future keeps running.
        let (registry, id) = registry_holding(ProcStatus::Stopped);
        let (mailbox, _inbox) = mpsc::channel(4);
        registry.begin_launch(id, mailbox);
        registry.remove_returning_handle(id); // the close/remove path took the handle

        let marker = Arc::new(());
        let weak = Arc::downgrade(&marker);
        let orphan = tokio::spawn(async move {
            let _held = marker; // released only when the future is dropped
            std::future::pending::<()>().await;
        });
        registry.attach_join(id, orphan);

        // The abort drops the cancelled task's future; give the runtime a few turns to run it.
        for _ in 0..8 {
            if weak.upgrade().is_none() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            weak.upgrade().is_none(),
            "the orphaned actor task is aborted, not left running"
        );
    }

    #[test]
    fn exhaust_holds_only_a_crashed_process() {
        // The exhaust edge is legal from Crashed alone, so the atomic guard refuses every
        // other state — a concurrent user restart is never clobbered into RestartExhausted.
        for resting in [
            ProcStatus::Stopped,
            ProcStatus::Starting,
            ProcStatus::Running,
        ] {
            let (registry, id) = registry_holding(resting);
            assert!(!registry.exhaust_if_crashed(id));
            assert_eq!(registry.status(id), Some(resting));
        }

        let (registry, id) = registry_holding(ProcStatus::Crashed);
        assert!(registry.exhaust_if_crashed(id));
        assert_eq!(registry.status(id), Some(ProcStatus::RestartExhausted));
        // Idempotent: once held, a second call is a no-op (no spurious re-exhaust).
        assert!(!registry.exhaust_if_crashed(id));
    }

    #[test]
    fn command_id_by_name_finds_the_command_in_its_project_only() {
        // `registry_holding` registers a Command labelled "x" in project 1.
        let (registry, id) = registry_holding(ProcStatus::Stopped);
        let project = ProjectId::from_raw(1);
        assert_eq!(registry.command_id_by_name(project, "x"), Some(id));
        assert_eq!(registry.command_id_by_name(project, "other"), None);
        assert_eq!(
            registry.command_id_by_name(ProjectId::from_raw(2), "x"),
            None
        );
    }

    #[test]
    fn update_command_spec_replaces_fields_but_leaves_status_untouched() {
        let (registry, id) = registry_holding(ProcStatus::Running);
        let launch = SpawnSpec {
            command: "npm run start".into(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            size: PtySize::default(),
        };
        assert!(registry.update_command_spec(
            id,
            "Web".into(),
            launch.clone(),
            None,
            false,
            false,
            Vec::new(),
            true,
        ));
        let view = registry.view(id).expect("view");
        assert_eq!(view.label, "Web");
        assert!(view.requires_trust);
        assert_eq!(
            view.status,
            ProcStatus::Running,
            "a spec update never touches a running process's status"
        );
        // A missing process reports no update.
        assert!(!registry.update_command_spec(
            ProcessId::from_raw(999),
            "x".into(),
            launch,
            None,
            false,
            false,
            Vec::new(),
            false,
        ));
    }

    #[test]
    fn remove_if_resting_drops_a_resting_process_but_keeps_an_active_one() {
        let (resting, id) = registry_holding(ProcStatus::Stopped);
        assert!(resting.remove_if_resting(id));
        assert_eq!(resting.status(id), None, "a resting process is dropped");

        let (running, id) = registry_holding(ProcStatus::Running);
        assert!(!running.remove_if_resting(id));
        assert_eq!(
            running.status(id),
            Some(ProcStatus::Running),
            "a running process is left untouched"
        );
    }

    #[test]
    fn a_monitoring_update_after_the_group_ends_is_dropped() {
        // While running, the process is on a known group and accepts monitoring updates.
        let (registry, id) = registry_holding(ProcStatus::Running);
        registry.set_pgid(id, Some(4242));
        assert!(registry.set_ports(id, 4242, vec![8080]));
        assert!(registry.set_ready(id, 4242, false));

        // The child is reaped: the group ends, clearing ports and readiness.
        registry.set_pgid(id, None);

        // A scan or wait that began before the stop now lands late, still carrying the old
        // pgid. It must be dropped — a resting process never resurrects ports or readiness.
        assert!(!registry.set_ports(id, 4242, vec![8080]));
        assert!(!registry.set_ready(id, 4242, true));
        let view = registry
            .snapshot()
            .into_iter()
            .find(|v| v.id == id)
            .unwrap();
        assert!(
            view.ports.is_empty(),
            "ports stay cleared after the group ends"
        );
        assert_eq!(view.ready, Readiness::Ungated);
    }
}
