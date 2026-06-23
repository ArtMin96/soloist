//! Process supervision (context C2): the registry, the supervised actor, and the
//! command API the façade routes to.
//!
//! The [`Supervisor`] is the single owner of process lifecycle. Adapters never spawn
//! or signal processes themselves — they route through the façade to one of these
//! commands, so "restart" (and the trust gate that guards it) is implemented exactly
//! once for the UI, MCP, and HTTP/CLI alike. The trust gate is enforced *here*, in the
//! core, on every start/restart path: an untrusted command variant cannot run.
//!
//! This root module holds the per-process lifecycle (`start`/`stop`/`restart`) and the
//! launch primitive the rest of the context shares. Cohesive concerns live in submodules:
//! `registration` (the [`Registration`] input), `bulk` (project-wide start/stop/restart +
//! [`StartSummary`]), `terminal_io` (the per-process output/input surface), `reconcile`
//! (orphan adoption), `actor`/`registry`/`adopt` (the runtime machinery).

mod actor;
mod adopt;
mod bulk;
mod monitoring;
mod reconcile;
mod registration;
mod registry;
mod restart;
mod terminal_io;

use std::path::PathBuf;
use std::sync::Arc;

use crate::events::{DomainEvent, EventBus};
use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{
    Clock, CorePorts, LockReleaser, OrphanControl, ProcessSpawner, RuntimeState, SpawnSpec,
    Spawned, StoreError, TrustRepo,
};
use crate::process::{ProcStatus, ProcessView, Readiness};
use crate::terminal::Terminals;

use actor::{ActorMsg, ActorPorts, OrphanIdentity};
use registry::{ActorHandle, Registry};
use restart::RestartPolicy;

pub use bulk::StartSummary;
pub use registration::Registration;

/// Per-actor mailbox capacity. Tiny on purpose: at most a couple of control messages
/// are ever in flight for one process, and a bounded channel honours the no-unbounded
/// rule.
const MAILBOX_CAPACITY: usize = 4;

/// Why a supervisor command failed.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("no such process: {0}")]
    NotFound(ProcessId),
    #[error("command is not trusted to run in this project")]
    Untrusted,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The process supervisor (context C2). Cheap to share behind an `Arc`.
pub struct Supervisor {
    spawner: Arc<dyn ProcessSpawner>,
    clock: Arc<dyn Clock>,
    trust: Arc<dyn TrustRepo>,
    locks: Arc<dyn LockReleaser>,
    runtime: Arc<dyn RuntimeState>,
    orphan_control: Arc<dyn OrphanControl>,
    bus: EventBus,
    registry: Registry,
    terminals: Terminals,
    restart_policy: RestartPolicy,
}

impl Supervisor {
    /// Builds a supervisor from the core port set, reading the ports it owns. The bus is
    /// shared with the façade so adapters see process events alongside config events;
    /// `runtime` persists running process groups and `orphan_control` operates on them
    /// for orphan adoption.
    pub fn new(ports: &CorePorts, bus: EventBus) -> Self {
        Self {
            spawner: ports.spawner.clone(),
            clock: ports.clock.clone(),
            trust: ports.trust.clone(),
            locks: ports.locks.clone(),
            runtime: ports.runtime.clone(),
            orphan_control: ports.orphan_control.clone(),
            bus,
            registry: Registry::default(),
            terminals: Terminals::default(),
            restart_policy: RestartPolicy::default(),
        }
    }

    /// The current process read model.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        self.registry.snapshot()
    }

    /// The display label of a process by id, `None` if it is no longer registered. A focused
    /// read for consumers (the notification reactor) that need one label, not the whole
    /// snapshot.
    pub fn label_of(&self, id: ProcessId) -> Option<String> {
        self.registry.label_of(id)
    }

    /// One process's read-model row by id, `None` if it is no longer registered. The focused
    /// counterpart to [`snapshot`](Self::snapshot) for consumers that need a single process.
    pub fn view(&self, id: ProcessId) -> Option<ProcessView> {
        self.registry.view(id)
    }

    /// The managed process whose live OS group leader is `pgid`, if any — the home process of
    /// a caller whose connecting peer runs in that group. The identity gate uses it to
    /// authenticate a session's binding and project scope against the kernel-reported peer
    /// process group, so a caller can only scope to its own process tree.
    pub fn process_at_pgid(&self, pgid: i32) -> Option<ProcessId> {
        self.registry.process_at_pgid(pgid)
    }

    /// Test-only: assigns a synthetic live process group to a registered process, standing in
    /// for the group a real spawn would create, so identity/scope tests can authenticate a
    /// session to the process without spinning up a real PTY. Never compiled into a release.
    #[cfg(any(test, feature = "testing"))]
    pub fn assign_test_group(&self, id: ProcessId, pgid: i32) {
        self.registry.set_pgid(id, Some(pgid));
    }

    /// Registers a process as `Stopped` without starting it, announcing it on the bus.
    pub fn register(&self, registration: Registration) -> ProcessId {
        let id = ProcessId::next();
        let Registration {
            project,
            kind,
            label,
            launch,
            project_root,
            trust_variant,
            auto_start,
            auto_restart,
            restart_when_changed,
        } = registration;
        let requires_trust = self.requires_trust(project, trust_variant.as_ref());
        let view = ProcessView {
            id,
            project,
            kind,
            label: label.clone(),
            status: ProcStatus::Stopped,
            exit_code: None,
            requires_trust,
            ports: Vec::new(),
            ready: Readiness::Ungated,
        };
        self.registry.add(
            view,
            launch,
            project_root,
            trust_variant,
            auto_start,
            auto_restart,
            restart_when_changed,
        );
        self.bus.publish(DomainEvent::ProcessSpawned {
            id,
            project,
            kind,
            label,
            status: ProcStatus::Stopped,
            requires_trust,
        });
        id
    }

    /// Whether a command variant still needs trust before it can run. Terminals and
    /// agents are ungated (`None`). A store read failure fails closed (treated as
    /// needing trust), matching the start gate, which also refuses what it cannot verify.
    fn requires_trust(&self, project: ProjectId, variant: Option<&Hash>) -> bool {
        match variant {
            Some(hash) => !self.trust.is_trusted(project, hash).unwrap_or(false),
            None => false,
        }
    }

    /// Records that `variant`'s commands in `project` are now trusted, clearing the
    /// read-model flag so they become startable. The durable trust write happens in the
    /// trust store (see [`crate::facade::Facade::trust_command`]); this only reflects it.
    pub fn mark_trusted(&self, project: ProjectId, variant: &Hash) {
        self.registry.mark_variant_trusted(project, variant);
    }

    /// Starts a process. A trust-gated command whose variant is not trusted is refused
    /// (untrusted cannot run by any path). Starting an already-active
    /// process is a no-op.
    pub fn start(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let info = self
            .registry
            .describe(id)
            .ok_or(SupervisorError::NotFound(id))?;
        if info.status.is_active() {
            return Ok(());
        }
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
        // A user-initiated start is an explicit retry: clear any crash-restart history so
        // a previously exhausted command starts with a fresh rate-limit window.
        self.restart_policy.forget(id);
        self.launch_actor(id, info.launch, None);
        Ok(())
    }

    /// Requests a graceful stop. Returns whether an active process was messaged; a
    /// resting or already-finished process reports `false`.
    pub fn stop(&self, id: ProcessId) -> bool {
        match self.registry.status(id) {
            Some(status) if status.is_active() => {
                if let Some(mailbox) = self.registry.mailbox(id) {
                    let _ = mailbox.try_send(ActorMsg::Stop);
                }
                true
            }
            _ => false,
        }
    }

    /// Restarts a process: a running one is told to cycle in place; a stopped one is
    /// started. Trust is re-checked on either path.
    pub fn restart(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let info = self
            .registry
            .describe(id)
            .ok_or(SupervisorError::NotFound(id))?;
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
        // A user-initiated restart is an explicit retry — reset crash tracking, as a stop
        // would (the auto-restart path relaunches directly and never clears).
        self.restart_policy.forget(id);
        if info.status.is_active() {
            if let Some(mailbox) = self.registry.mailbox(id) {
                let _ = mailbox.try_send(ActorMsg::Restart);
            }
        } else {
            self.launch_actor(id, info.launch, None);
        }
        Ok(())
    }

    /// Stops every live process across all projects and awaits each actor's exit, so no
    /// children leak on app quit (the deterministic-shutdown contract). Wired into the
    /// Tauri shell's exit event so a normal quit reaps every process group.
    pub async fn shutdown(&self) {
        // Latch the policy closed first so no crash during teardown is auto-restarted: the
        // children we are about to reap must not be relaunched.
        self.restart_policy.begin_shutdown();
        // Reap in passes until none remain. A crash whose auto-restart check slipped in
        // just before the latch became visible can still spawn one last actor while we
        // reap; the latch stops the reactor from launching any further, so the set is
        // finite and this converges.
        loop {
            let mut joins = Vec::new();
            for id in self.registry.with_live_actor() {
                if let Some(ActorHandle { mailbox, join }) = self.registry.take_handle(id) {
                    let _ = mailbox.try_send(ActorMsg::Stop);
                    joins.push(join);
                }
            }
            if joins.is_empty() {
                break;
            }
            for join in joins {
                let _ = join.await;
            }
        }
    }

    /// Refuses an untrusted trust-gated command; ungated processes always pass.
    fn guard_trust(
        &self,
        project: ProjectId,
        variant: Option<&Hash>,
    ) -> Result<(), SupervisorError> {
        if let Some(variant) = variant {
            if !self.trust.is_trusted(project, variant)? {
                return Err(SupervisorError::Untrusted);
            }
        }
        Ok(())
    }

    /// Atomically claims a resting process, moves it into `Starting`, and spawns its
    /// actor. `initial` is the pre-built handle for an adopted orphan (the first
    /// iteration uses it instead of spawning); a normal launch passes `None`. Returns
    /// `false` without spawning if the process was already active — closing the start
    /// race when two callers target the same process at once.
    fn launch_actor(&self, id: ProcessId, launch: SpawnSpec, initial: Option<Spawned>) -> bool {
        let Some(from) = self.registry.begin_launch(id) else {
            return false;
        };
        self.bus.publish(DomainEvent::ProcessStatusChanged {
            id,
            from,
            to: ProcStatus::Starting,
            exit_code: None,
        });
        let identity = self
            .registry
            .identity(id)
            .unwrap_or_else(|| OrphanIdentity {
                project_root: PathBuf::new(),
                name: String::new(),
            });
        let (mailbox, inbox) = tokio::sync::mpsc::channel(MAILBOX_CAPACITY);
        let join = actor::spawn(id, launch, identity, self.actor_ports(), inbox, initial);
        self.registry.set_handle(id, ActorHandle { mailbox, join });
        true
    }

    fn actor_ports(&self) -> ActorPorts {
        ActorPorts {
            spawner: self.spawner.clone(),
            clock: self.clock.clone(),
            locks: self.locks.clone(),
            runtime: self.runtime.clone(),
            bus: self.bus.clone(),
            registry: self.registry.clone(),
            terminals: self.terminals.clone(),
        }
    }
}

/// Applies one FSM transition and, when legal, updates the registry and publishes the
/// delta. Shared by the supervisor (reading `from` from the registry) and the actor
/// (passing its own local status mirror). An illegal transition is refused — the
/// current state is returned unchanged — because the FSM is the contract.
pub(crate) fn apply_transition(
    registry: &Registry,
    bus: &EventBus,
    id: ProcessId,
    from: ProcStatus,
    to: ProcStatus,
    exit_code: Option<i32>,
) -> ProcStatus {
    match from.transition(to) {
        Ok(new) => {
            registry.set_status(id, new, exit_code);
            bus.publish(DomainEvent::ProcessStatusChanged {
                id,
                from,
                to: new,
                exit_code,
            });
            new
        }
        Err(_) => from,
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessKind;
    use crate::supervisor::test_support::{
        command_spec, harness, next_change, next_to, spawn_spec, status_of, terminal, wait_all,
        PROJECT,
    };
    use crate::testing::FakeSpawner;
    use std::path::Path;
    use std::time::Duration;
    use tokio::sync::broadcast::error::RecvError;

    /// A duration safely past the actor's SIGTERM→SIGKILL grace window.
    const PAST_GRACE: Duration = Duration::from_secs(6);

    #[tokio::test]
    async fn start_then_stop_runs_the_full_lifecycle_via_the_mock_clock() {
        let mut h = harness(FakeSpawner::exits_on_kill());
        let id = terminal(&h.sup, "sleep 60");

        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);

        // The fake child ignores SIGTERM, so the grace window must elapse (no real
        // time) before SIGKILL reaps it.
        assert!(h.sup.stop(id));
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopping);
        tokio::task::yield_now().await;
        h.clock.advance(PAST_GRACE);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, id), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn an_untrusted_command_cannot_run_by_any_path() {
        let mut h = harness(FakeSpawner::exits_on_kill());
        let spec = command_spec("npm run dev", true);
        let id = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Web",
            &spec,
        ));

        // Every start path refuses an untrusted command variant.
        assert!(matches!(h.sup.start(id), Err(SupervisorError::Untrusted)));
        assert!(matches!(h.sup.restart(id), Err(SupervisorError::Untrusted)));
        let summary = h.sup.start_all(PROJECT).expect("start_all");
        assert!(summary.started.is_empty());
        assert_eq!(summary.skipped_untrusted, vec![id]);
        assert_eq!(status_of(&h.sup, id), ProcStatus::Stopped);

        // Once the exact variant is trusted, it starts.
        h.trust
            .set_trusted(PROJECT, &spec.variant_hash())
            .expect("trust");
        h.sup.start(id).expect("start trusted");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);
    }

    #[tokio::test]
    async fn terminals_and_agents_run_without_a_trust_record() {
        let mut h = harness(FakeSpawner::exits_on_kill());
        let term = h.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Terminal,
            "shell",
            spawn_spec("bash"),
        ));
        let agent = h.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Agent,
            "claude",
            spawn_spec("claude"),
        ));

        h.sup.start(term).expect("start terminal");
        h.sup.start(agent).expect("start agent");
        wait_all(&mut h.rx, &[term, agent], ProcStatus::Running).await;
    }

    #[tokio::test]
    async fn a_clean_exit_is_stopped_with_its_code() {
        let mut h = harness(FakeSpawner::exits_with_code(0));
        let id = terminal(&h.sup, "true");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);
        assert_eq!(next_change(&mut h.rx).await, (ProcStatus::Stopped, Some(0)));
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_a_crash_with_its_code() {
        let mut h = harness(FakeSpawner::exits_with_code(3));
        let id = terminal(&h.sup, "false");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);
        assert_eq!(next_change(&mut h.rx).await, (ProcStatus::Crashed, Some(3)));
    }

    #[tokio::test]
    async fn an_external_signal_is_a_crash() {
        let mut h = harness(FakeSpawner::killed_by_signal(9));
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);
        assert_eq!(next_change(&mut h.rx).await, (ProcStatus::Crashed, None));
    }

    #[tokio::test]
    async fn a_user_stop_is_stopped_not_crashed_even_when_sigkilled() {
        let mut h = harness(FakeSpawner::exits_on_kill());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);

        h.sup.stop(id);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopping);
        tokio::task::yield_now().await;
        h.clock.advance(PAST_GRACE);
        // Reaped by SIGKILL, yet classified as a clean stop because we initiated it.
        assert_eq!(next_change(&mut h.rx).await, (ProcStatus::Stopped, None));
        assert_eq!(status_of(&h.sup, id), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn restart_cycles_a_running_process_in_place() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);

        h.sup.restart(id).expect("restart");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Restarting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);
    }

    #[tokio::test]
    async fn a_closed_process_releases_its_locks() {
        // A graceful stop releases locks.
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let stopped = terminal(&h.sup, "sleep 60");
        h.sup.start(stopped).expect("start");
        wait_all(&mut h.rx, &[stopped], ProcStatus::Running).await;
        h.sup.stop(stopped);
        wait_all(&mut h.rx, &[stopped], ProcStatus::Stopped).await;
        tokio::task::yield_now().await;
        assert!(h.locks.released().contains(&stopped));

        // So does a crash.
        let mut h = harness(FakeSpawner::exits_with_code(2));
        let crashed = terminal(&h.sup, "false");
        h.sup.start(crashed).expect("start");
        wait_all(&mut h.rx, &[crashed], ProcStatus::Crashed).await;
        tokio::task::yield_now().await;
        assert!(h.locks.released().contains(&crashed));
    }

    #[tokio::test]
    async fn a_panicking_process_is_isolated_and_the_supervisor_survives() {
        let mut h = harness(FakeSpawner::panics_after_running());
        let id = terminal(&h.sup, "boom");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);
        // The panic is caught and surfaced as Crashed, and its locks are released.
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Crashed);
        tokio::task::yield_now().await;
        assert!(h.locks.released().contains(&id));

        // The supervisor is still alive: another process runs to completion.
        let mut h2 = harness(FakeSpawner::exits_on_kill());
        let other = terminal(&h2.sup, "sleep 60");
        h2.sup.start(other).expect("start");
        wait_all(&mut h2.rx, &[other], ProcStatus::Running).await;
    }

    #[tokio::test]
    async fn shutdown_stops_and_reaps_every_live_process() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let one = terminal(&h.sup, "sleep 60");
        let two = terminal(&h.sup, "sleep 60");
        h.sup.start(one).expect("start one");
        h.sup.start(two).expect("start two");
        wait_all(&mut h.rx, &[one, two], ProcStatus::Running).await;

        // Shutdown awaits every actor, so on return both children are reaped and at
        // rest — the no-leak-on-quit contract, proven without racing the event stream.
        h.sup.shutdown().await;
        assert_eq!(status_of(&h.sup, one), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, two), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn pty_output_is_buffered_and_a_title_is_published() {
        let chunk = b"\x1b]0;my title\x07hello\n".to_vec();
        let mut h = harness(FakeSpawner::streams_then_exits(vec![chunk.clone()]));
        let id = terminal(&h.sup, "agent");
        h.sup.start(id).expect("start");

        // Consume the lifecycle, capturing any terminal title set along the way. The
        // process is at rest once it reaches Stopped, with its buffers fully drained.
        let mut title = None;
        loop {
            match h.rx.recv().await {
                Ok(DomainEvent::TerminalTitleChanged { id: got, title: t }) if got == id => {
                    title = Some(t);
                }
                Ok(DomainEvent::ProcessStatusChanged { id: got, to, .. })
                    if got == id && to == ProcStatus::Stopped =>
                {
                    break
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }

        assert_eq!(title.as_deref(), Some("my title"));
        // The rendered line has the OSC stripped; the raw scrollback keeps every byte.
        assert_eq!(
            h.sup.rendered(id).expect("rendered").lines,
            vec!["hello".to_string()]
        );
        assert_eq!(h.sup.pty_scrollback(id).expect("scrollback"), chunk);
    }

    #[tokio::test]
    async fn attach_replays_the_scrollback_of_a_finished_process() {
        let chunk = b"output line\n".to_vec();
        let mut h = harness(FakeSpawner::streams_then_exits(vec![chunk.clone()]));
        let id = terminal(&h.sup, "cmd");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;

        // Attaching after the process stopped still replays its raw scrollback.
        let (scrollback, _live) = h.sup.attach_pty(id).expect("a terminal channel");
        assert_eq!(scrollback, chunk);
    }

    #[tokio::test]
    async fn input_to_an_unknown_process_is_not_found() {
        let h = harness(FakeSpawner::exits_on_kill());
        let unknown = ProcessId::from_raw(999);
        assert!(matches!(
            h.sup.write_stdin(unknown, b"x".to_vec()).await,
            Err(SupervisorError::NotFound(_))
        ));
        assert!(matches!(
            h.sup.resize(unknown, 80, 24).await,
            Err(SupervisorError::NotFound(_))
        ));
    }
}
