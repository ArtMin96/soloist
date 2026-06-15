//! Process supervision (context C2): the registry, the supervised actor, and the
//! command API the façade routes to.
//!
//! The [`Supervisor`] is the single owner of process lifecycle. Adapters never spawn
//! or signal processes themselves — they route through the façade to one of these
//! commands, so "restart" (and the trust gate that guards it) is implemented exactly
//! once for the UI, MCP, and HTTP/CLI alike. The trust gate is enforced *here*, in the
//! core, on every start/restart path: an untrusted command variant cannot run.

mod actor;
mod adopt;
mod registry;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::orphans::{classify, OrphanFate, OrphanInfo, OrphanReport};
use crate::ports::{
    Clock, LockReleaser, OrphanControl, ProcessSpawner, PtySize, RuntimeState, SpawnSpec, Spawned,
    StoreError, TrustRepo,
};
use crate::process::{ProcStatus, ProcessKind, ProcessView};
use crate::terminal::{PtyChunk, PtyInput, RenderedScreen, Terminals};

use actor::{ActorMsg, ActorPorts, OrphanIdentity};
use registry::{ActorHandle, Registry};

/// Per-actor mailbox capacity. Tiny on purpose: at most a couple of control messages
/// are ever in flight for one process, and a bounded channel honours the no-unbounded
/// rule.
const MAILBOX_CAPACITY: usize = 4;

/// How to create a managed process.
pub struct Registration {
    pub project: ProjectId,
    pub kind: ProcessKind,
    pub label: String,
    pub launch: SpawnSpec,
    /// The project root, recorded as part of the process's orphan-adoption identity.
    pub project_root: PathBuf,
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
                size: PtySize::default(),
            },
            project_root: root.to_path_buf(),
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
        // A launched terminal/agent has no project-root command identity; its working
        // directory stands in, so a leftover never matches a configured command.
        let project_root = launch.working_dir.clone();
        Self {
            project,
            kind,
            label: label.into(),
            launch,
            project_root,
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
    runtime: Arc<dyn RuntimeState>,
    orphan_control: Arc<dyn OrphanControl>,
    bus: EventBus,
    registry: Registry,
    terminals: Terminals,
}

impl Supervisor {
    /// Builds a supervisor over the given ports and event bus. The bus is shared with
    /// the façade so adapters see process events alongside config events. `runtime`
    /// persists running process groups and `orphan_control` operates on them for
    /// orphan adoption.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spawner: Arc<dyn ProcessSpawner>,
        clock: Arc<dyn Clock>,
        trust: Arc<dyn TrustRepo>,
        locks: Arc<dyn LockReleaser>,
        runtime: Arc<dyn RuntimeState>,
        orphan_control: Arc<dyn OrphanControl>,
        bus: EventBus,
    ) -> Self {
        Self {
            spawner,
            clock,
            trust,
            locks,
            runtime,
            orphan_control,
            bus,
            registry: Registry::default(),
            terminals: Terminals::default(),
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
            project_root,
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
        self.registry
            .add(view, launch, project_root, trust_variant, auto_start);
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
        if info.status.is_active() {
            return Ok(());
        }
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
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
        if info.status.is_active() {
            if let Some(mailbox) = self.registry.mailbox(id) {
                let _ = mailbox.try_send(ActorMsg::Restart);
            }
        } else {
            self.launch_actor(id, info.launch, None);
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
                if self.launch_actor(candidate.id, candidate.launch, None) {
                    summary.started.push(candidate.id);
                }
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
    /// children leak on app quit (the deterministic-shutdown contract). Wired into the
    /// Tauri shell's exit event so a normal quit reaps every process group.
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

    /// Attaches a viewer to a process's terminal output (detach/attach): returns the
    /// raw scrollback to replay plus a live receiver to stream, captured atomically so
    /// there is no gap or duplicate between them. `None` if the process has never been
    /// started. Detaching is just dropping the receiver — the process keeps running and
    /// other viewers are unaffected.
    pub fn attach_pty(&self, id: ProcessId) -> Option<(Vec<u8>, broadcast::Receiver<PtyChunk>)> {
        self.terminals.attach(id)
    }

    /// A process's raw byte scrollback snapshot (control sequences included), for output
    /// tools that read without attaching. `None` if it has never been started.
    pub fn pty_scrollback(&self, id: ProcessId) -> Option<Vec<u8>> {
        self.terminals.scrollback(id)
    }

    /// A process's rendered output snapshot (escape sequences applied to plain text).
    /// `None` if the process has never been started.
    pub fn rendered(&self, id: ProcessId) -> Option<RenderedScreen> {
        self.terminals.rendered(id)
    }

    /// Writes bytes (typed text or raw control sequences) to a running process's PTY.
    /// Returns [`SupervisorError::NotFound`] for a process with no terminal; input to a
    /// process that has since stopped is delivered best-effort and dropped.
    pub async fn write_stdin(&self, id: ProcessId, data: Vec<u8>) -> Result<(), SupervisorError> {
        self.send_input(id, PtyInput::Write(data)).await
    }

    /// Resizes a running process's PTY so the child sees the new dimensions (and a
    /// `SIGWINCH`). Best-effort, as for [`Supervisor::write_stdin`].
    pub async fn resize(&self, id: ProcessId, cols: u16, rows: u16) -> Result<(), SupervisorError> {
        self.send_input(id, PtyInput::Resize(PtySize { cols, rows }))
            .await
    }

    /// Routes one input message to a process's owning actor over its bounded input
    /// channel, applying backpressure rather than dropping when the actor is busy.
    async fn send_input(&self, id: ProcessId, input: PtyInput) -> Result<(), SupervisorError> {
        match self.terminals.input(id) {
            // A closed channel (the process has since stopped) is a harmless no-op.
            Some(sender) => {
                let _ = sender.send(input).await;
                Ok(())
            }
            None => Err(SupervisorError::NotFound(id)),
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

    /// Reconciles the runtime-state file against live process groups on launch: prunes
    /// dead records, adopts live groups that match a registered command (re-attaching
    /// them as running), and surfaces unmatched live groups via [`DomainEvent::OrphansFound`]
    /// for a user Kill/Leave decision. Registered commands must be in place before this
    /// is called so matches can be found. Must run within a `tokio` runtime.
    pub fn reconcile_orphans(&self) -> OrphanReport {
        let records = self.runtime.load().unwrap_or_default();
        let fates = classify(
            records,
            |pgid| self.orphan_control.is_alive(pgid),
            |record| {
                self.registry.find_resting_match(
                    &record.project_root,
                    &record.name,
                    &record.command,
                )
            },
        );

        let mut report = OrphanReport::default();
        let mut surfaced = Vec::new();
        for fate in fates {
            match fate {
                OrphanFate::Adopt { record, target } => {
                    if self.adopt_orphan(target, record.pgid) {
                        report.adopted.push(target);
                    }
                }
                OrphanFate::Surface(record) => surfaced.push(OrphanInfo::from(&record)),
                OrphanFate::Prune(record) => {
                    let _ = self.runtime.forget(record.pgid);
                    report.pruned += 1;
                }
            }
        }
        if !surfaced.is_empty() {
            self.bus.publish(DomainEvent::OrphansFound {
                orphans: surfaced.clone(),
            });
            report.surfaced = surfaced;
        }
        report
    }

    /// Re-attaches a leftover process group `pgid` to the resting registered process
    /// `target`, running it through the normal actor over a synthesized handle.
    fn adopt_orphan(&self, target: ProcessId, pgid: i32) -> bool {
        let Some(launch) = self.registry.describe(target).map(|info| info.launch) else {
            return false;
        };
        let spawned = adopt::adopt(pgid, self.orphan_control.clone(), self.clock.clone());
        self.launch_actor(target, launch, Some(spawned))
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
mod tests {
    use super::*;
    use crate::ports::OrphanRecord;
    use crate::testing::{
        FakeOrphanControl, FakeRuntimeState, FakeSpawner, FakeTrustRepo, MockClock,
        RecordingLockReleaser,
    };
    use std::collections::{BTreeMap, HashSet};
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::RecvError;

    /// A duration safely past the actor's SIGTERM→SIGKILL grace window.
    const PAST_GRACE: Duration = Duration::from_secs(6);
    /// A duration past the adopted-process liveness poll, so a death is observed.
    const PAST_POLL: Duration = Duration::from_secs(2);
    const PROJECT: ProjectId = ProjectId::from_raw(1);

    struct Harness {
        sup: Supervisor,
        trust: Arc<FakeTrustRepo>,
        locks: RecordingLockReleaser,
        clock: MockClock,
        runtime: Arc<FakeRuntimeState>,
        orphans: Arc<FakeOrphanControl>,
        rx: broadcast::Receiver<DomainEvent>,
    }

    fn harness(spawner: FakeSpawner) -> Harness {
        let bus = EventBus::new(256);
        let rx = bus.subscribe();
        let trust = Arc::new(FakeTrustRepo::new());
        let locks = RecordingLockReleaser::new();
        let clock = MockClock::new();
        let runtime = Arc::new(FakeRuntimeState::new());
        let orphans = Arc::new(FakeOrphanControl::new());
        let sup = Supervisor::new(
            Arc::new(spawner),
            Arc::new(clock.clone()),
            trust.clone(),
            Arc::new(locks.clone()),
            runtime.clone(),
            orphans.clone(),
            bus,
        );
        Harness {
            sup,
            trust,
            locks,
            clock,
            runtime,
            orphans,
            rx,
        }
    }

    fn spawn_spec(command: &str) -> SpawnSpec {
        SpawnSpec {
            command: command.into(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            size: PtySize::default(),
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

    fn orphan_record(name: &str, command: &str, pgid: i32) -> OrphanRecord {
        OrphanRecord {
            project_root: PathBuf::from("/p"),
            name: name.into(),
            command: command.into(),
            pgid,
        }
    }

    async fn next_orphans(rx: &mut broadcast::Receiver<DomainEvent>) -> Vec<OrphanInfo> {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::OrphansFound { orphans }) => return orphans,
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    #[tokio::test]
    async fn a_running_process_is_recorded_then_forgotten() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        // While running, the process group is in the runtime-state file.
        assert_eq!(h.runtime.records().len(), 1, "recorded while running");

        h.sup.stop(id);
        wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;
        tokio::task::yield_now().await;
        assert!(h.runtime.records().is_empty(), "forgotten once reaped");
    }

    #[tokio::test]
    async fn reconcile_adopts_a_matching_live_orphan_then_can_stop_it() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        // A registered, resting command and a leftover group that matches it.
        let spec = command_spec("npm run dev", false);
        let id = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Web",
            &spec,
        ));
        h.runtime.seed(orphan_record("Web", "npm run dev", 555));
        h.orphans.set_alive(555);

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.adopted, vec![id], "matched live orphan is adopted");
        assert!(report.surfaced.is_empty());
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        // Stopping the adopted process signals its group and clears its record.
        h.sup.stop(id);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopping);
        tokio::task::yield_now().await;
        h.clock.advance(PAST_POLL);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopped);
        assert!(
            h.orphans.signalled().contains(&(555, false)),
            "SIGTERM to group"
        );
        assert!(h.runtime.records().is_empty(), "record cleared on stop");
    }

    #[tokio::test]
    async fn reconcile_surfaces_an_unmatched_live_orphan() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        h.runtime.seed(orphan_record("stray", "weird --serve", 777));
        h.orphans.set_alive(777);

        let report = h.sup.reconcile_orphans();
        assert!(report.adopted.is_empty());
        assert_eq!(report.surfaced.len(), 1);
        assert_eq!(report.surfaced[0].pgid, 777);

        // The same candidate is announced for a user Kill/Leave decision.
        let announced = next_orphans(&mut h.rx).await;
        assert_eq!(announced.len(), 1);
        assert_eq!(announced[0].name, "stray");
    }

    #[tokio::test]
    async fn reconcile_prunes_a_dead_orphan() {
        let h = harness(FakeSpawner::exits_on_terminate());
        // Recorded but no longer alive (never marked alive in the fake control).
        h.runtime.seed(orphan_record("gone", "old", 888));

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.pruned, 1);
        assert!(report.adopted.is_empty());
        assert!(report.surfaced.is_empty());
        assert!(h.runtime.records().is_empty(), "stale record pruned");
    }
}
