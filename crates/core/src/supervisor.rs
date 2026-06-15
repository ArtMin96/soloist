//! Process supervision (context C2): the registry, the supervised actor, and the
//! command API the façade routes to.
//!
//! The [`Supervisor`] is the single owner of process lifecycle. Adapters never spawn
//! or signal processes themselves — they route through the façade to one of these
//! commands, so "restart" (and the trust gate that guards it) is implemented exactly
//! once for the UI, MCP, and HTTP/CLI alike. The trust gate is enforced *here*, in the
//! core, on every start/restart path: an untrusted command variant cannot run.

mod actor;
mod registry;

use std::path::Path;
use std::sync::Arc;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{Clock, LockReleaser, ProcessSpawner, SpawnSpec, StoreError, TrustRepo};
use crate::process::{ProcStatus, ProcessKind, ProcessView};

use actor::{ActorMsg, ActorPorts};
use registry::{ActorHandle, Registry};

/// Per-actor mailbox capacity. Tiny on purpose: at most a couple of control messages
/// are ever in flight for one process, and a bounded channel honours the no-unbounded
/// rule (plan/04 §8).
const MAILBOX_CAPACITY: usize = 4;

/// How to create a managed process.
pub struct Registration {
    pub project: ProjectId,
    pub kind: ProcessKind,
    pub label: String,
    pub launch: SpawnSpec,
    /// `Some(variant)` makes this a trust-gated command; `None` (terminals and agents,
    /// which the user launches directly) is never trust-gated.
    pub trust_variant: Option<Hash>,
    pub auto_start: bool,
}

impl Registration {
    /// A trust-gated [`ProcessKind::Command`] from a `solo.yml` [`ProcessSpec`], with
    /// its working directory resolved against the project root.
    pub fn command(
        project: ProjectId,
        root: &Path,
        name: impl Into<String>,
        spec: &ProcessSpec,
    ) -> Self {
        Self {
            project,
            kind: ProcessKind::Command,
            label: name.into(),
            launch: SpawnSpec {
                command: spec.command.clone(),
                working_dir: spec.resolved_working_dir(root),
                env: spec.env.clone(),
            },
            trust_variant: Some(spec.variant_hash()),
            auto_start: spec.auto_start,
        }
    }

    /// An ungated process (a terminal or agent) launched directly — never trust-gated
    /// and never eligible for auto-start.
    pub fn launched(
        project: ProjectId,
        kind: ProcessKind,
        label: impl Into<String>,
        launch: SpawnSpec,
    ) -> Self {
        Self {
            project,
            kind,
            label: label.into(),
            launch,
            trust_variant: None,
            auto_start: false,
        }
    }
}

/// The outcome of a bulk start: what was started, and what was skipped because its
/// command variant is not trusted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartSummary {
    pub started: Vec<ProcessId>,
    pub skipped_untrusted: Vec<ProcessId>,
}

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
    bus: EventBus,
    registry: Registry,
}

impl Supervisor {
    /// Builds a supervisor over the given ports and event bus. The bus is shared with
    /// the façade so adapters see process events alongside config events.
    pub fn new(
        spawner: Arc<dyn ProcessSpawner>,
        clock: Arc<dyn Clock>,
        trust: Arc<dyn TrustRepo>,
        locks: Arc<dyn LockReleaser>,
        bus: EventBus,
    ) -> Self {
        Self {
            spawner,
            clock,
            trust,
            locks,
            bus,
            registry: Registry::default(),
        }
    }

    /// The current process read model.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        self.registry.snapshot()
    }

    /// Registers a process as `Stopped` without starting it, announcing it on the bus.
    pub fn register(&self, registration: Registration) -> ProcessId {
        let id = ProcessId::next();
        let Registration {
            project,
            kind,
            label,
            launch,
            trust_variant,
            auto_start,
        } = registration;
        let view = ProcessView {
            id,
            project,
            kind,
            label: label.clone(),
            status: ProcStatus::Stopped,
            exit_code: None,
        };
        self.registry.add(view, launch, trust_variant, auto_start);
        self.bus.publish(DomainEvent::ProcessSpawned {
            id,
            kind,
            label,
            status: ProcStatus::Stopped,
        });
        id
    }

    /// Starts a process. A trust-gated command whose variant is not trusted is refused
    /// (closing A6: untrusted cannot run by any path). Starting an already-active
    /// process is a no-op.
    pub fn start(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let info = self
            .registry
            .describe(id)
            .ok_or(SupervisorError::NotFound(id))?;
        if is_active(info.status) {
            return Ok(());
        }
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
        self.launch_actor(id, info.launch);
        Ok(())
    }

    /// Requests a graceful stop. Returns whether a live actor was messaged.
    pub fn stop(&self, id: ProcessId) -> bool {
        match self.registry.mailbox(id) {
            Some(mailbox) => {
                let _ = mailbox.try_send(ActorMsg::Stop);
                true
            }
            None => false,
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
        if is_active(info.status) {
            if let Some(mailbox) = self.registry.mailbox(id) {
                let _ = mailbox.try_send(ActorMsg::Restart);
            }
        } else {
            self.launch_actor(id, info.launch);
        }
        Ok(())
    }

    /// Starts every trusted `auto_start` command in a project; untrusted candidates are
    /// reported, not run.
    pub fn start_all(&self, project: ProjectId) -> Result<StartSummary, SupervisorError> {
        let mut summary = StartSummary::default();
        for candidate in self.registry.auto_start_candidates(project) {
            let trusted = match &candidate.trust_variant {
                Some(variant) => self.trust.is_trusted(project, variant)?,
                None => true,
            };
            if trusted {
                self.launch_actor(candidate.id, candidate.launch);
                summary.started.push(candidate.id);
            } else {
                summary.skipped_untrusted.push(candidate.id);
            }
        }
        Ok(summary)
    }

    /// Requests a graceful stop of every live process in a project.
    pub fn stop_all(&self, project: ProjectId) {
        for id in self.registry.live_in(project) {
            self.stop(id);
        }
    }

    /// Restarts every currently-running process in a project (trusted only; an
    /// untrusted one is skipped).
    pub fn restart_running(&self, project: ProjectId) -> Result<(), SupervisorError> {
        for id in self.registry.running_in(project) {
            match self.restart(id) {
                Ok(()) | Err(SupervisorError::Untrusted) | Err(SupervisorError::NotFound(_)) => {}
                Err(err @ SupervisorError::Store(_)) => return Err(err),
            }
        }
        Ok(())
    }

    /// Stops every live process across all projects and awaits each actor's exit, so no
    /// children leak on app quit (the deterministic-shutdown contract, plan/04 §8).
    pub async fn shutdown(&self) {
        let mut joins = Vec::new();
        for id in self.registry.with_live_actor() {
            if let Some(ActorHandle { mailbox, join }) = self.registry.take_handle(id) {
                let _ = mailbox.try_send(ActorMsg::Stop);
                joins.push(join);
            }
        }
        for join in joins {
            let _ = join.await;
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

    /// Moves a resting process into `Starting` and spawns its actor.
    fn launch_actor(&self, id: ProcessId, launch: SpawnSpec) {
        if let Some(from) = self.registry.status(id) {
            apply_transition(
                &self.registry,
                &self.bus,
                id,
                from,
                ProcStatus::Starting,
                None,
            );
        }
        let (mailbox, inbox) = tokio::sync::mpsc::channel(MAILBOX_CAPACITY);
        let join = actor::spawn(id, launch, self.actor_ports(), inbox);
        self.registry.set_handle(id, ActorHandle { mailbox, join });
    }

    fn actor_ports(&self) -> ActorPorts {
        ActorPorts {
            spawner: self.spawner.clone(),
            clock: self.clock.clone(),
            locks: self.locks.clone(),
            bus: self.bus.clone(),
            registry: self.registry.clone(),
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

/// Whether a status means an actor is (or should be) live for this process.
fn is_active(status: ProcStatus) -> bool {
    matches!(
        status,
        ProcStatus::Starting | ProcStatus::Running | ProcStatus::Restarting | ProcStatus::Stopping
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeSpawner, FakeTrustRepo, MockClock, RecordingLockReleaser};
    use std::collections::{BTreeMap, HashSet};
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::RecvError;

    /// A duration safely past the actor's SIGTERM→SIGKILL grace window.
    const PAST_GRACE: Duration = Duration::from_secs(6);
    const PROJECT: ProjectId = ProjectId::from_raw(1);

    struct Harness {
        sup: Supervisor,
        trust: Arc<FakeTrustRepo>,
        locks: RecordingLockReleaser,
        clock: MockClock,
        rx: broadcast::Receiver<DomainEvent>,
    }

    fn harness(spawner: FakeSpawner) -> Harness {
        let bus = EventBus::new(256);
        let rx = bus.subscribe();
        let trust = Arc::new(FakeTrustRepo::new());
        let locks = RecordingLockReleaser::new();
        let clock = MockClock::new();
        let sup = Supervisor::new(
            Arc::new(spawner),
            Arc::new(clock.clone()),
            trust.clone(),
            Arc::new(locks.clone()),
            bus,
        );
        Harness {
            sup,
            trust,
            locks,
            clock,
            rx,
        }
    }

    fn spawn_spec(command: &str) -> SpawnSpec {
        SpawnSpec {
            command: command.into(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
        }
    }

    fn command_spec(command: &str, auto_start: bool) -> ProcessSpec {
        ProcessSpec {
            command: command.into(),
            working_dir: None,
            auto_start,
            auto_restart: false,
            restart_when_changed: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    fn terminal(sup: &Supervisor, command: &str) -> ProcessId {
        sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Terminal,
            "shell",
            spawn_spec(command),
        ))
    }

    async fn next_to(rx: &mut broadcast::Receiver<DomainEvent>) -> ProcStatus {
        next_change(rx).await.0
    }

    async fn next_change(rx: &mut broadcast::Receiver<DomainEvent>) -> (ProcStatus, Option<i32>) {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { to, exit_code, .. }) => {
                    return (to, exit_code)
                }
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    async fn wait_all(
        rx: &mut broadcast::Receiver<DomainEvent>,
        ids: &[ProcessId],
        target: ProcStatus,
    ) {
        let mut remaining: HashSet<ProcessId> = ids.iter().copied().collect();
        while !remaining.is_empty() {
            match rx.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { id, to, .. }) if to == target => {
                    remaining.remove(&id);
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    fn status_of(sup: &Supervisor, id: ProcessId) -> ProcStatus {
        sup.snapshot()
            .into_iter()
            .find(|view| view.id == id)
            .map(|view| view.status)
            .expect("process is registered")
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
    async fn start_all_starts_only_trusted_auto_start_commands() {
        let mut h = harness(FakeSpawner::exits_on_kill());
        let auto_trusted = command_spec("run a", true);
        let auto_untrusted = command_spec("run b", true);
        let manual_trusted = command_spec("run c", false);

        let a = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "A",
            &auto_trusted,
        ));
        let b = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "B",
            &auto_untrusted,
        ));
        let c = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "C",
            &manual_trusted,
        ));
        let term = terminal(&h.sup, "bash");

        h.trust
            .set_trusted(PROJECT, &auto_trusted.variant_hash())
            .expect("trust a");
        h.trust
            .set_trusted(PROJECT, &manual_trusted.variant_hash())
            .expect("trust c");

        let summary = h.sup.start_all(PROJECT).expect("start_all");
        assert_eq!(
            summary.started,
            vec![a],
            "only the trusted auto-start command"
        );
        assert_eq!(summary.skipped_untrusted, vec![b]);

        wait_all(&mut h.rx, &[a], ProcStatus::Running).await;
        // The non-auto command, the untrusted one, and the terminal stay put.
        assert_eq!(status_of(&h.sup, b), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, c), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, term), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn stop_all_stops_every_live_process_in_the_project() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let one = terminal(&h.sup, "sleep 60");
        let two = terminal(&h.sup, "sleep 60");
        h.sup.start(one).expect("start one");
        h.sup.start(two).expect("start two");
        wait_all(&mut h.rx, &[one, two], ProcStatus::Running).await;

        h.sup.stop_all(PROJECT);
        wait_all(&mut h.rx, &[one, two], ProcStatus::Stopped).await;
        assert_eq!(status_of(&h.sup, one), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, two), ProcStatus::Stopped);
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
    async fn restart_running_restarts_the_running_processes() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        h.sup.restart_running(PROJECT).expect("restart_running");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Restarting);
    }
}
