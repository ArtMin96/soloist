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
mod lifecycle;
mod monitoring;
mod reconcile;
mod registration;
mod registry;
mod restart;
mod terminal_io;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::task::JoinHandle;

use std::collections::BTreeMap;

use crate::events::{DomainEvent, EventBus};
use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{
    Clock, LockReleaser, OrphanControl, ProcessSpawner, RuntimeState, SpawnSpec, Spawned,
    StoreError, TrustRepo,
};
use crate::process::{ProcStatus, ProcessView, Readiness};
use crate::shellenv::{ShellEnv, ShellEnvProbe};
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

/// Ceiling on consecutive [`shutdown`](Supervisor::shutdown) passes that find only mid-launch
/// entries (mailbox installed, join not yet attached) with no join to await. The launch window is
/// a handful of synchronous statements, so the join attaches within a pass or two; this bounds the
/// wait (the no-unbounded-retry rule) should a launcher wedge before attaching one. Generous, as a
/// yield-per-pass is cheap and the cap is only a safety backstop, never reached in practice.
pub(super) const MAX_SHUTDOWN_IDLE_PASSES: u32 = 1_000;

/// Why a supervisor command failed.
#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("no such process: {0}")]
    NotFound(ProcessId),
    #[error("command is not trusted to run in this project")]
    Untrusted,
    #[error("process {0} has no last session to resume")]
    NotResumable(ProcessId),
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
    /// Resolves the environment each process launches with — the captured login-shell
    /// environment layered under the process's own `env`. Shared by every actor so the
    /// shell is captured at most once per cache window.
    shell_env: Arc<ShellEnv>,
}

/// Exactly the ports process supervision drives. Named here, by the context that uses them, so
/// the supervisor never has to know the shape of the whole-core port set — the composition root
/// projects this out of it. Without that split a context would depend on the assembler that
/// depends on every context, which is a cycle.
pub struct SupervisorPorts {
    pub spawner: Arc<dyn ProcessSpawner>,
    pub clock: Arc<dyn Clock>,
    pub trust: Arc<dyn TrustRepo>,
    pub locks: Arc<dyn LockReleaser>,
    pub runtime: Arc<dyn RuntimeState>,
    pub orphan_control: Arc<dyn OrphanControl>,
    pub shell_env_probe: Arc<dyn ShellEnvProbe>,
    /// The app's own environment, captured at the composition root, that the login-shell
    /// resolver layers under each process's `env`.
    pub app_env: BTreeMap<String, String>,
}

impl Supervisor {
    /// Builds a supervisor over the ports it drives. The bus is shared with the façade so
    /// adapters see process events alongside config events; `runtime` persists running process
    /// groups and `orphan_control` operates on them for orphan adoption.
    pub fn new(ports: SupervisorPorts, bus: EventBus) -> Self {
        let shell_env = Arc::new(ShellEnv::new(
            ports.shell_env_probe,
            ports.clock.clone(),
            ports.app_env,
        ));
        Self {
            spawner: ports.spawner,
            clock: ports.clock,
            trust: ports.trust,
            locks: ports.locks,
            runtime: ports.runtime,
            orphan_control: ports.orphan_control,
            bus,
            registry: Registry::default(),
            terminals: Terminals::default(),
            restart_policy: RestartPolicy::default(),
            shell_env,
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
            resume_command,
        } = registration;
        let requires_trust = self.requires_trust(project, trust_variant.as_ref());
        // A resume command was composed for this process iff its provider can resume its last
        // session, so its presence is the single source for the resumable read-model flag.
        let resumable = resume_command.is_some();
        let view = ProcessView {
            id,
            project,
            kind,
            label: label.clone(),
            status: ProcStatus::Stopped,
            exit_code: None,
            requires_trust,
            resumable,
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
            resume_command,
        );
        self.bus.publish(DomainEvent::ProcessSpawned {
            id,
            project,
            kind,
            label,
            status: ProcStatus::Stopped,
            requires_trust,
            resumable,
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
        // Create the mailbox up front and install it as the launch is claimed, so a stop or
        // shutdown that lands in the window before the actor is scheduled still reaches it — the
        // command is neither lost nor falsely reported delivered, and shutdown can see a
        // mid-launch process to reap. The join handle is attached below once the task exists.
        let (mailbox, inbox) = tokio::sync::mpsc::channel(MAILBOX_CAPACITY);
        let Some(from) = self.registry.begin_launch(id, mailbox) else {
            return false;
        };
        // Open the terminal channel synchronously, before spawning the actor, so a viewer that
        // attaches in the window between this launch and the actor being scheduled finds a live
        // channel instead of "process has not started". The actor receives the actor-facing half
        // rather than opening it itself; a relaunch reuses the existing buffers.
        let terminal = self.terminals.open(id);
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
        let join = actor::spawn(
            id,
            launch,
            identity,
            self.actor_ports(),
            inbox,
            initial,
            terminal,
        );
        self.registry.attach_join(id, join);
        true
    }

    fn actor_ports(&self) -> ActorPorts {
        ActorPorts {
            spawner: self.spawner.clone(),
            clock: self.clock.clone(),
            locks: self.locks.clone(),
            runtime: self.runtime.clone(),
            orphan_control: self.orphan_control.clone(),
            bus: self.bus.clone(),
            registry: self.registry.clone(),
            shell_env: self.shell_env.clone(),
        }
    }
}

/// Messages a live actor to stop and hands back its join handle to await, if it has one — the
/// shared reap step behind [`Supervisor::close`] (which awaits the one) and
/// [`Supervisor::shutdown`] (which messages every actor, then awaits them together). Dropping the
/// mailbox at the end of this call is itself a stop signal for a mid-launch actor whose join is
/// not yet attached (its `recv` unblocks to `None`). Best-effort and tolerant: a full mailbox is
/// ignored and awaiting an already-finished actor returns at once.
pub(super) fn signal_stop(handle: ActorHandle) -> Option<JoinHandle<()>> {
    let ActorHandle { mailbox, join } = handle;
    let _ = mailbox.try_send(ActorMsg::Stop);
    join
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
        // The FSM is the contract, so an illegal edge is a bug in a caller, not an expected
        // input: refuse it (leaving the state unchanged) but trace it, so a regression that
        // would silently desync the registry from the actor's status mirror is diagnosable
        // instead of invisible.
        Err(_) => {
            tracing::warn!(
                process = id.get(),
                ?from,
                ?to,
                "refused an illegal process status transition"
            );
            from
        }
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod lifecycle_tests;

#[cfg(test)]
mod resume_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PROCESS_ID_ENV;
    use crate::ports::PtySize;
    use crate::process::ProcessKind;
    use crate::supervisor::test_support::{
        command_spec, harness, harness_with_shell_env, next_change, next_to, spawn_spec, status_of,
        terminal, wait_all, PROJECT,
    };
    use crate::testing::{FakeShellEnvProbe, FakeSpawner, PANIC_FAKE_PGID};
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::time::Duration;
    use tokio::sync::broadcast::error::RecvError;

    /// Builds an environment map from `(key, value)` pairs.
    fn env_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// A duration safely past the actor's SIGTERM→SIGKILL grace window.
    const PAST_GRACE: Duration = Duration::from_secs(6);

    #[tokio::test]
    async fn a_spawn_layers_the_captured_shell_env_under_the_process_overrides() {
        // The login-shell capture exposes a version-manager bin dir and a key the process
        // also sets; the process's own value must win, the captured-only key must carry
        // through, and the injected process id must ride along.
        let captured = env_map(&[
            ("NVM_BIN", "/home/dev/.nvm/versions/node/bin"),
            ("SHARED", "from-shell"),
        ]);
        let (spawner, recorder) = FakeSpawner::records_spec_env();
        let mut h = harness_with_shell_env(
            spawner,
            Arc::new(FakeShellEnvProbe::returning(captured)),
            BTreeMap::new(),
        );

        let mut spec = spawn_spec("sleep 60");
        spec.env = env_map(&[("SHARED", "from-process"), ("FOO", "bar")]);
        let id = h.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Terminal,
            "shell",
            spec,
        ));
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        let spawned = recorder.lock().expect("recorder").first().cloned();
        let spawned = spawned.expect("the process was spawned once");
        assert_eq!(
            spawned.get("NVM_BIN"),
            Some(&"/home/dev/.nvm/versions/node/bin".to_string()),
            "the captured version-manager dir reaches the spawn"
        );
        assert_eq!(
            spawned.get("SHARED"),
            Some(&"from-process".to_string()),
            "the process's own env wins over the captured shell env"
        );
        assert_eq!(spawned.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(
            spawned.get(PROCESS_ID_ENV),
            Some(&id.get().to_string()),
            "the injected process id rides along"
        );
    }

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
    async fn a_blocking_stdin_write_does_not_wedge_the_actor() {
        // A child that has stopped reading its stdin blocks every PTY write forever. Input
        // is applied off the select loop, so the stuck write stalls only the input pump —
        // the actor stays responsive and a stop still tears the process down. Bounded by a
        // timeout so a regression (input awaited inline again) fails here, not hangs.
        let (spawner, write_blocking) = FakeSpawner::blocks_on_input();
        let mut h = harness(spawner);
        let id = terminal(&h.sup, "sleep 60");

        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);

        // Send input and wait until the write is actually blocking, so the assertion below
        // is a real test of responsiveness while a write is stuck, not a race.
        h.sup
            .write_stdin(id, b"typing into a deaf child".to_vec())
            .await
            .expect("write accepted into the bounded input channel");
        write_blocking.notified().await;

        // The fake exits on SIGTERM, so a working stop reaches Stopped with no grace elapse;
        // a wedged actor would never process the stop and the timeout would fire.
        assert!(h.sup.stop(id), "an active process is messaged");
        let stopped = tokio::time::timeout(Duration::from_secs(5), async {
            while next_to(&mut h.rx).await != ProcStatus::Stopped {}
        })
        .await;
        assert!(
            stopped.is_ok(),
            "a stuck stdin write must not wedge the actor's stop path"
        );
        assert_eq!(status_of(&h.sup, id), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn best_effort_delivery_to_a_deaf_child_drops_instead_of_blocking() {
        // Autonomous timer delivery uses the non-blocking `try_write_stdin`: a child that has
        // stopped draining its stdin fills its bounded input channel, but delivery must drop
        // rather than await, so one deaf agent cannot stall the scheduler for every other agent.
        // Bounded by a timeout so a regression (delivery awaiting a full channel) fails here.
        let (spawner, write_blocking) = FakeSpawner::blocks_on_input();
        let mut h = harness(spawner);
        let id = terminal(&h.sup, "sleep 60");

        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);

        // Wedge the input pump on a blocking write, then flood far past the channel's capacity.
        h.sup
            .write_stdin(id, b"first".to_vec())
            .await
            .expect("write accepted into the bounded input channel");
        write_blocking.notified().await;

        let flooded = tokio::time::timeout(Duration::from_secs(5), async {
            for _ in 0..1024 {
                h.sup
                    .try_write_stdin(id, b"timer body".to_vec())
                    .expect("a live process accepts (and may drop) best-effort input");
            }
        })
        .await;
        assert!(
            flooded.is_ok(),
            "best-effort delivery to a deaf child must drop, never block the caller"
        );

        // A process with no terminal is still a not-found, as for the blocking path.
        assert!(matches!(
            h.sup
                .try_write_stdin(ProcessId::from_raw(999), b"x".to_vec()),
            Err(SupervisorError::NotFound(_))
        ));
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
    async fn a_spawn_failure_surfaces_the_reason_in_the_terminal_and_crashes() {
        // A command that cannot be spawned (missing binary, bad working dir) crashes — but the
        // reason must reach its terminal, not vanish, so the user sees why instead of an empty
        // pane that flaps to RestartExhausted with no explanation.
        let mut h = harness(FakeSpawner::fails_to_spawn("no such file or directory"));
        let id = terminal(&h.sup, "definitely-not-a-binary");
        h.sup.start(id).expect("start");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(next_change(&mut h.rx).await, (ProcStatus::Crashed, None));

        let rendered = h
            .sup
            .rendered(id)
            .expect("the crashed process kept its terminal");
        let text = rendered.lines.join("\n");
        assert!(
            text.contains("failed to start") && text.contains("no such file or directory"),
            "the spawn error is written to the terminal: {text:?}"
        );
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
    async fn attach_pty_is_available_synchronously_after_start() {
        // The empty-pane race: a viewer that attaches in the window between `start` returning and
        // the actor being scheduled must still find a live terminal channel. The channel is opened
        // synchronously as the process launches, so `attach_pty` is total for a launched process
        // and never returns `None` (which the UI surfaces as a "Press Start" overlay on a process
        // that is actually running).
        let h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        // Deliberately no `.await` first: the spawned actor has not been polled yet. When the
        // channel was opened lazily inside the actor body this was `None`; opening it in the
        // synchronous launch path makes it `Some`.
        assert!(
            h.sup.attach_pty(id).is_some(),
            "a launched process has a terminal channel before its actor runs"
        );
    }

    #[tokio::test]
    async fn a_never_started_process_has_no_terminal_channel() {
        // A resting, never-started process must still report `None`, so the UI shows the
        // "Press Start" overlay rather than an empty live pane. Only a launch opens the channel.
        let h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        assert!(
            h.sup.attach_pty(id).is_none(),
            "a never-started process has no terminal channel to attach to"
        );
    }

    #[tokio::test]
    async fn closing_a_process_frees_its_terminal_channel() {
        // A removed process must not leak its terminal buffers. While running it has a channel to
        // attach to; after close (a full removal, unlike a stop, which keeps the scrollback
        // readable) the channel is gone, so a long session of opens and closes does not grow RSS.
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
        assert!(
            h.sup.attach_pty(id).is_some(),
            "a running process has a terminal channel"
        );

        h.sup.close(id).await.expect("close");
        assert!(
            h.sup.attach_pty(id).is_none(),
            "closing frees the terminal channel"
        );
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
        // The child the panicked task left behind is reaped (SIGKILL to its group) and its
        // orphan record forgotten, so a crash auto-restart cannot spawn a second beside it.
        assert!(
            h.orphans.signalled().contains(&(PANIC_FAKE_PGID, true)),
            "the leftover child's group is SIGKILLed on the panic path"
        );
        assert!(
            h.runtime.records().is_empty(),
            "the leftover child's orphan record is forgotten"
        );

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
    async fn a_stop_in_the_launch_window_is_delivered_and_stops_without_spawning() {
        // The launch-window race: `start` claims the launch and installs the actor mailbox
        // synchronously, but the actor task has not been polled yet (no `.await` here). A stop
        // that lands in that window must reach the actor — reported delivered, not silently
        // dropped — and the process must actually stop. Because the mailbox is installed as the
        // launch is claimed, the stop reaches the actor, and the actor's pre-spawn drain honors
        // it before spawning: the status goes straight Starting -> Stopping -> Stopped, never
        // Running, so no child is ever spawned.
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");

        h.sup.start(id).expect("start");
        assert!(
            h.sup.stop(id),
            "a stop in the launch window is delivered, not dropped"
        );

        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
        assert_eq!(
            next_to(&mut h.rx).await,
            ProcStatus::Stopping,
            "the actor honors the queued stop before spawning — never reaches Running"
        );
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, id), ProcStatus::Stopped);
        assert!(
            h.runtime.records().is_empty(),
            "no child was ever spawned in the launch window"
        );
    }

    #[tokio::test]
    async fn shutdown_reaps_a_process_still_in_its_launch_window() {
        // A shutdown racing a launch must not miss a process whose actor has not been polled yet:
        // the mailbox installed at claim time makes it visible to `with_live_actor`, and the
        // pre-spawn drain honors the shutdown's stop, so no child is spawned and none survives the
        // quit. The status settles at Stopped (via Stopping), never having reached Running.
        let h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");

        h.sup.start(id).expect("start");
        // The actor is spawned but unpolled — the launch window. Shutdown must still reap it.
        h.sup.shutdown().await;

        assert_eq!(status_of(&h.sup, id), ProcStatus::Stopped);
        assert!(
            h.runtime.records().is_empty(),
            "no child survives the quit via the launch window"
        );
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
    async fn a_resize_reaches_the_running_pty() {
        // A resize routed to a live process is applied to its PTY, not dropped — the input path
        // carries resizes so the FE can keep the PTY winsize in step with the pane.
        let (spawner, log) = FakeSpawner::records_resizes();
        let mut h = harness(spawner);
        let id = terminal(&h.sup, "tui");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        h.sup.resize(id, 120, 40).await.expect("resize");
        log.resize_applied().await;

        assert_eq!(
            log.resizes(),
            vec![PtySize {
                cols: 120,
                rows: 40
            }]
        );
        // The first spawn created its PTY at the default, before any viewer resized it.
        assert_eq!(log.spawns(), vec![PtySize::default()]);
    }

    #[tokio::test]
    async fn a_respawn_relaunches_the_pty_at_the_last_resize_size() {
        // After a viewer resizes the pane, a relaunch (here an in-place restart) re-creates the
        // PTY at that size rather than resetting to the 80×24 default — otherwise a relaunched
        // TUI renders into the wrong dimensions until the next resize.
        let (spawner, log) = FakeSpawner::records_resizes();
        let mut h = harness(spawner);
        let id = terminal(&h.sup, "tui");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        h.sup.resize(id, 120, 40).await.expect("resize");
        // Wait until the pump has applied the resize, so the last size is recorded before restart.
        log.resize_applied().await;

        h.sup.restart(id).expect("restart");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        // Two spawns: the first at the default, the second re-created at the remembered size.
        assert_eq!(
            log.spawns(),
            vec![
                PtySize::default(),
                PtySize {
                    cols: 120,
                    rows: 40
                }
            ]
        );
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

    /// A minimal `tracing` subscriber that only records whether a `WARN` event was emitted — just
    /// enough to prove the illegal-transition path traces rather than staying silent, without a
    /// subscriber dependency.
    #[derive(Clone, Default)]
    struct WarnFlag(Arc<std::sync::atomic::AtomicBool>);

    impl WarnFlag {
        fn was_warned(&self) -> bool {
            self.0.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl tracing::Subscriber for WarnFlag {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            if *event.metadata().level() == tracing::Level::WARN {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }

    #[test]
    fn an_illegal_transition_is_refused_and_traced() {
        // `Stopped -> Running` skips `Starting`: the FSM forbids it. `apply_transition` must
        // refuse it — return the unchanged state and publish no delta — but trace it, so a
        // regression that would desync the registry from the actor's status mirror is
        // diagnosable rather than invisible.
        let registry = Registry::default();
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let id = ProcessId::next();

        let warned = WarnFlag::default();
        let result = tracing::subscriber::with_default(warned.clone(), || {
            apply_transition(
                &registry,
                &bus,
                id,
                ProcStatus::Stopped,
                ProcStatus::Running,
                None,
            )
        });

        assert_eq!(
            result,
            ProcStatus::Stopped,
            "an illegal transition leaves the state unchanged"
        );
        assert!(warned.was_warned(), "an illegal transition is traced");
        assert!(
            rx.try_recv().is_err(),
            "no status delta is published for a refused transition"
        );
    }
}
