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
use crate::process::{ProcStatus, ProcessKind, ProcessView};
use crate::sync::lock;

use super::actor::{ActorMsg, OrphanIdentity};

/// A live actor's control surface: a bounded mailbox to message it and the join
/// handle awaited at shutdown to confirm its child was reaped.
pub(crate) struct ActorHandle {
    pub(crate) mailbox: mpsc::Sender<ActorMsg>,
    pub(crate) join: JoinHandle<()>,
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
    pub(crate) fn add(
        &self,
        view: ProcessView,
        launch: SpawnSpec,
        project_root: PathBuf,
        trust_variant: Option<Hash>,
        auto_start: bool,
        auto_restart: bool,
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
                entry.view.ready = None;
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

    /// Updates a process's readiness gate, returning whether it changed (so the caller only
    /// announces real transitions).
    pub(crate) fn set_ready(&self, id: ProcessId, ready: Option<bool>) -> bool {
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id) {
            Some(entry) if entry.view.ready != ready => {
                entry.view.ready = ready;
                true
            }
            _ => false,
        }
    }

    /// Updates a process's discovered listening ports, returning whether the set actually
    /// changed (so the port scanner only announces real changes). The ports are stored
    /// sorted by the caller; a no-op update returns `false`.
    pub(crate) fn set_ports(&self, id: ProcessId, ports: Vec<u16>) -> bool {
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id) {
            Some(entry) if entry.view.ports != ports => {
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

    /// Atomically claims the right to launch a resting process: if it is not already
    /// active, transitions it to `Starting` under the lock and returns its prior
    /// status; if it is already active (another launch won, or it is running), returns
    /// `None`. Setting the status under the same lock as the check is what makes the
    /// supervisor's start path race-free without holding the lock across the spawn.
    pub(crate) fn begin_launch(&self, id: ProcessId) -> Option<ProcStatus> {
        let mut guard = lock(&self.inner);
        let entry = guard.get_mut(&id)?;
        let from = entry.view.status;
        if from.is_active() {
            return None;
        }
        let next = from.transition(ProcStatus::Starting).ok()?;
        entry.view.status = next;
        entry.view.exit_code = None;
        Some(from)
    }

    /// Atomically holds a still-crashed process in [`ProcStatus::RestartExhausted`]: if it
    /// is currently `Crashed`, transitions it and returns `true`; otherwise leaves it
    /// untouched and returns `false`. Because the FSM permits `RestartExhausted` from no
    /// state but `Crashed`, a concurrent user start/restart (now `Starting` or running) is
    /// never clobbered — the transition simply fails. The caller publishes the status
    /// delta after this returns, as with [`Registry::begin_launch`].
    pub(crate) fn exhaust_if_crashed(&self, id: ProcessId) -> bool {
        let mut guard = lock(&self.inner);
        let Some(entry) = guard.get_mut(&id) else {
            return false;
        };
        match entry.view.status.transition(ProcStatus::RestartExhausted) {
            Ok(next) => {
                entry.view.status = next;
                entry.view.exit_code = None;
                true
            }
            Err(_) => false,
        }
    }

    /// Stores the handle of a freshly launched actor.
    pub(crate) fn set_handle(&self, id: ProcessId, handle: ActorHandle) {
        let mut guard = lock(&self.inner);
        if let Some(entry) = guard.get_mut(&id) {
            entry.handle = Some(handle);
        }
    }

    /// Clones a live actor's mailbox to message it without removing its handle, so a
    /// restart keeps the same actor task.
    pub(crate) fn mailbox(&self, id: ProcessId) -> Option<mpsc::Sender<ActorMsg>> {
        let guard = lock(&self.inner);
        guard
            .get(&id)
            .and_then(|entry| entry.handle.as_ref().map(|h| h.mailbox.clone()))
    }

    /// Removes and returns a live actor's handle, used at shutdown to message then
    /// await it.
    pub(crate) fn take_handle(&self, id: ProcessId) -> Option<ActorHandle> {
        let mut guard = lock(&self.inner);
        guard.get_mut(&id).and_then(|entry| entry.handle.take())
    }

    /// Every process that still holds an actor handle — the shutdown set. Messaging
    /// then awaiting each reaps any child still alive; it is a harmless no-op for an
    /// actor that already finished but whose handle was not reclaimed.
    pub(crate) fn with_live_actor(&self) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.handle.is_some())
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Active processes within `project` — the targets of `stop_all`.
    pub(crate) fn live_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.view.project == project && entry.view.status.is_active())
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Running processes within `project` — the targets of `restart_running`.
    pub(crate) fn running_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.project == project && entry.view.status == ProcStatus::Running
            })
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Stopped, `auto_start` commands within `project` — the candidates `start_all`
    /// trust-checks before launching.
    pub(crate) fn auto_start_candidates(&self, project: ProjectId) -> Vec<Candidate> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.project == project
                    && entry.view.kind == ProcessKind::Command
                    && entry.auto_start
                    && entry.view.status == ProcStatus::Stopped
            })
            .map(|entry| Candidate {
                id: entry.view.id,
                trust_variant: entry.trust_variant,
                launch: entry.launch.clone(),
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
            ports: Vec::new(),
            ready: None,
        };
        let launch = SpawnSpec {
            command: "x".into(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            size: PtySize::default(),
        };
        registry.add(view, launch, PathBuf::from("/"), None, false, false);
        registry.set_status(id, status, None);
        (registry, id)
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
}
