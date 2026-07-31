use super::*;
use crate::agents::{AgentKind, AgentTool, PromptMode};
use crate::composition::CorePorts;
use crate::config::parse;
use crate::events::DomainEvent;
use crate::facade::scoped_process::MAX_INPUT_WAIT;
use crate::facade::scoped_process::MAX_REPORT_BYTES;
use crate::ids::ProjectId;
use crate::ports::ProjectRepo;
use crate::ports::{Clock, TokioClock, TrustRepo};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::Registration;
use crate::sync::lock;
use crate::testing::{
    agent_registration, authentic_session, facade_recording_agent_launches, facade_with_agent_tool,
    terminal_registration, FakeAgentToolRepo, FakeProjectRepo, FakeSpawner, FakeTrustRepo,
    TEST_PEER_PGID,
};
use crate::PeerCredentials;
use async_trait::async_trait;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// A façade over in-memory fakes, sharing the trust repo so a test can grant trust.
fn facade() -> (Facade, Arc<FakeTrustRepo>) {
    let trust = Arc::new(FakeTrustRepo::new());
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            trust.clone(),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    );
    (facade, trust)
}

/// Registers a terminal in `project` and returns its id. A terminal is ungated, so it is
/// the simplest process to exercise the scope guard with.
fn terminal_in(facade: &Facade, project: ProjectId, name: &str) -> ProcessId {
    facade
        .supervisor()
        .register(terminal_registration(project, name, "sleep 60"))
}

/// Opens a session authenticated to `process` and binds it, as the UDS adapter would for an
/// MCP client running inside that process's group: [`authentic_session`] puts the caller in
/// the process's group, then this binds it, so the bind passes the façade's authenticity
/// check. The production scope path, without a real PTY.
fn scoped_to(facade: &Facade, process: ProcessId) -> SessionId {
    let session = authentic_session(facade, process, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(process)
        .expect("an authentic bind to the process the caller runs in");
    session
}

async fn wait_for(rx: &mut broadcast::Receiver<DomainEvent>, target: ProcStatus) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { to, .. }) if to == target => return,
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

#[tokio::test]
async fn an_in_scope_process_starts_and_stops() {
    let (facade, _trust) = facade();
    let mut rx = facade.subscribe();
    let project = ProjectId::from_raw(1);
    let id = terminal_in(&facade, project, "term");
    // The session's peer runs in the process's group, the same way a Soloist-launched agent's
    // session resolves its project from the process it runs in.
    let session = scoped_to(&facade, id);

    facade
        .scoped(session)
        .start_process(id)
        .expect("an in-scope terminal starts");
    wait_for(&mut rx, ProcStatus::Running).await;

    assert!(
        facade
            .scoped(session)
            .stop_process(id)
            .expect("in-scope stop"),
        "a running process reports it was live"
    );
    wait_for(&mut rx, ProcStatus::Stopped).await;
}

#[test]
fn an_unknown_process_is_refused() {
    let (facade, _trust) = facade();
    let session = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade
            .scoped(session)
            .start_process(ProcessId::from_raw(999)),
        Err(ScopedActionError::UnknownProcess)
    ));
}

#[test]
fn acting_without_a_project_in_scope_is_refused() {
    let (facade, _trust) = facade();
    // The process exists, so the guard passes the existence check, but the unbound session
    // has no project loaded, selected, or bound — its scope is ambiguous.
    let id = terminal_in(&facade, ProjectId::from_raw(1), "term");
    let session = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade.scoped(session).start_process(id),
        Err(ScopedActionError::NoProjectScope)
    ));
}

#[test]
fn another_projects_process_is_out_of_scope() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let elsewhere = terminal_in(&facade, ProjectId::from_raw(2), "elsewhere");
    let session = scoped_to(&facade, here);

    // The guard is shared by every action, so start, stop, and restart all refuse it.
    assert!(matches!(
        facade.scoped(session).start_process(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade.scoped(session).stop_process(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade.scoped(session).restart_process(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
}

/// A clock that records the duration it was asked to sleep and returns at once, so a test
/// asserts `send_input` clamped the wait with no real time passing.
#[derive(Clone, Default)]
struct RecordingClock {
    slept: Arc<Mutex<Option<Duration>>>,
}

#[async_trait]
impl Clock for RecordingClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn now_unix_millis(&self) -> u64 {
        0
    }

    async fn sleep(&self, dur: Duration) {
        *lock(&self.slept) = Some(dur);
    }
}

#[tokio::test]
async fn send_input_clamps_an_excessive_wait() {
    let clock = RecordingClock::default();
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(clock.clone()),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    );
    let mut rx = facade.subscribe();
    let id = terminal_in(&facade, ProjectId::from_raw(1), "term");
    let session = scoped_to(&facade, id);
    facade
        .scoped(session)
        .start_process(id)
        .expect("an in-scope start");
    wait_for(&mut rx, ProcStatus::Running).await;

    // A wait far beyond the cap is clamped to MAX_INPUT_WAIT before the clock ever sleeps, so a
    // remote caller cannot tie up the request (and the connection behind it) with a huge value.
    facade
        .scoped(session)
        .send_input(id, b"x".to_vec(), Some(Duration::from_secs(3600)))
        .await
        .expect("send_input succeeds");
    assert_eq!(*lock(&clock.slept), Some(MAX_INPUT_WAIT));
}

#[tokio::test]
async fn send_input_enforces_scope() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let elsewhere = terminal_in(&facade, ProjectId::from_raw(2), "elsewhere");
    let session = scoped_to(&facade, here);
    // send_input shares the one scope guard, so a cross-project target is refused too.
    assert!(matches!(
        facade
            .scoped(session)
            .send_input(elsewhere, b"x".to_vec(), None)
            .await,
        Err(ScopedActionError::OutOfScope)
    ));
}

#[test]
fn spawn_agent_without_a_project_in_scope_is_refused() {
    let (facade, _trust) = facade();
    let session = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade
            .scoped(session)
            .spawn_agent("Claude", Vec::new(), false),
        Err(SpawnAgentError::NoProjectScope)
    ));
}

#[test]
fn spawn_agent_with_an_unknown_tool_is_refused() {
    let (facade, _trust) = facade();
    // Scope to a process's project so a project is in scope; the tool name still does not
    // exist (the default facade registers no agent tools).
    let id = terminal_in(&facade, ProjectId::from_raw(1), "term");
    let session = scoped_to(&facade, id);
    assert!(matches!(
        facade
            .scoped(session)
            .spawn_agent("NoSuchTool", Vec::new(), false),
        Err(SpawnAgentError::Launch(LaunchAgentError::UnknownTool))
    ));
}

#[tokio::test]
async fn a_spawned_worker_cannot_spawn_its_own_worker() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");

    // The worker's own MCP client binds to it, exactly as the lead's did — but its spawn is
    // refused, and the refusal has no side effects: nothing new in the registry, no lineage.
    let worker_session = authentic_session(&facade, worker, TEST_PEER_PGID + 1);
    facade
        .scoped(worker_session)
        .bind_session_process(worker)
        .expect("an authentic bind to the worker");
    let registered_before = facade.snapshot().len();
    assert!(matches!(
        facade
            .scoped(worker_session)
            .spawn_agent("worker", Vec::new(), false),
        Err(SpawnAgentError::WorkerMayNotSpawn)
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

#[tokio::test]
async fn a_worker_that_never_binds_is_still_refused_a_spawn() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");

    // The worker's client connects from the worker's own process group but never binds, so
    // the session asserts no identity of its own. The group it connects from is not the
    // caller's to choose, and it is what the gate reads.
    let worker_session = authentic_session(&facade, worker, TEST_PEER_PGID + 1);
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade
            .scoped(worker_session)
            .spawn_agent("worker", Vec::new(), false),
        Err(SpawnAgentError::WorkerMayNotSpawn)
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

#[tokio::test]
async fn the_worker_gate_survives_the_group_the_caller_was_recognised_by_going_away() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");
    let worker_pgid = TEST_PEER_PGID + 1;
    let worker_session = authentic_session(&facade, worker, worker_pgid);
    facade
        .scoped(worker_session)
        .bind_session_process(worker)
        .expect("an authentic bind to the worker");

    // The worker's OS group is only live while its process is: a restart leaves the session's
    // original peer group owning nothing, so the group alone no longer names the caller.
    facade
        .supervisor()
        .assign_test_group(worker, worker_pgid + 100);

    assert!(matches!(
        facade
            .scoped(worker_session)
            .spawn_agent("worker", Vec::new(), false),
        Err(SpawnAgentError::WorkerMayNotSpawn)
    ));
}

#[tokio::test]
async fn the_worker_gate_outlives_a_closed_lead() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");
    let worker_session = authentic_session(&facade, worker, TEST_PEER_PGID + 1);
    facade
        .scoped(worker_session)
        .bind_session_process(worker)
        .expect("an authentic bind to the worker");

    facade
        .supervisor()
        .close(lead)
        .await
        .expect("close the lead");

    // The tree re-roots the worker, but the gate does not: a closed lead never promotes its
    // workers to spawners.
    assert!(matches!(
        facade
            .scoped(worker_session)
            .spawn_agent("worker", Vec::new(), false),
        Err(SpawnAgentError::WorkerMayNotSpawn)
    ));
}

/// The lineage parent recorded for `worker`, read through the tree the sidebar renders (an
/// edge counts only while both ends are live), or `None` when it reads back as a root.
fn lead_of(facade: &Facade, worker: ProcessId) -> Option<ProcessId> {
    facade
        .lineage_edges()
        .into_iter()
        .find(|edge| edge.child == worker)
        .map(|edge| edge.parent)
}

#[tokio::test]
async fn a_bound_leads_worker_nests_under_it() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);

    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");

    assert_eq!(lead_of(&facade, worker), Some(lead));
}

#[tokio::test]
async fn a_lead_that_never_bound_still_has_its_worker_nest_under_it() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    // The lead's client connects from the lead's own process group but never binds. The group
    // is not the caller's to choose and names it just as the gate reads it, so its spawn is a
    // worker of that lead — not a second root beside it.
    let lead_session = authentic_session(&facade, lead, TEST_PEER_PGID);

    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("an unbound lead spawns a worker");

    assert_eq!(lead_of(&facade, worker), Some(lead));
}

#[tokio::test]
async fn a_caller_soloist_cannot_name_spawns_a_root() {
    let (facade, _project) = facade_with_agent_tool();
    // An external caller: no binding and no managed process in its group, so nothing names it.
    // Its spawn has no parent to nest under and reads back as a root.
    let session = facade.open_session(PeerCredentials::unauthenticated());

    let worker = facade
        .scoped(session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("an external caller spawns into the sole project");

    assert_eq!(lead_of(&facade, worker), None);
}

/// Awaits `id` reaching `target`, ignoring every other process's transitions.
async fn wait_process_to(
    rx: &mut broadcast::Receiver<DomainEvent>,
    id: ProcessId,
    target: ProcStatus,
) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { id: got, to, .. })
                if got == id && to == target =>
            {
                return
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

/// Awaits `id` leaving the registry, failing rather than hanging if it never does.
async fn wait_process_removed(rx: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) {
    let removal = async {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessRemoved { id: got }) if got == id => return,
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(10), removal)
        .await
        .expect("the worker is closed once its run ends");
}

#[tokio::test]
async fn only_a_worker_spawned_to_close_when_done_leaves_the_registry() {
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    let mut rx = facade.subscribe();
    tokio::spawn(facade.auto_close_loop());

    let kept = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker to keep");
    let closed = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), true)
        .expect("a lead spawns a worker to close");
    for worker in [kept, closed] {
        wait_process_to(&mut rx, worker, ProcStatus::Running).await;
        facade
            .scoped(lead_session)
            .stop_process(worker)
            .expect("stop the worker");
    }
    // The closed one's removal proves the reactor has run past both stops.
    wait_process_removed(&mut rx, closed).await;

    let registered: Vec<ProcessId> = facade.snapshot().into_iter().map(|view| view.id).collect();
    assert!(
        registered.contains(&kept),
        "a finished worker stays listed by default, so its lead can still read it"
    );
    assert!(
        !registered.contains(&closed),
        "a worker spawned with close_when_done is forgotten once its run ends"
    );
}

#[tokio::test]
async fn a_spawned_worker_opens_on_its_orchestration_context() {
    let (facade, project, commands) = facade_recording_agent_launches();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    let mut rx = facade.subscribe();

    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");
    wait_process_to(&mut rx, worker, ProcStatus::Running).await;

    // The worker's CLI is launched with the briefing already submitted, so it starts knowing
    // which lead spawned it, which project it is in, and what it can reach.
    let launched = lock(&commands).first().cloned().expect("a launch");
    assert!(
        launched.contains("[SOLO ORCHESTRATION CONTEXT]"),
        "{launched}"
    );
    assert!(
        launched.contains(&format!("process #{lead}")),
        "the worker is told which process spawned it: {launched}"
    );
    assert!(
        launched.contains("whoami"),
        "the worker is told how to resolve its own identity: {launched}"
    );
}

/// A running lead with a running worker spawned by it, plus the buffer the lead's PTY input is
/// recorded into — the shape every report test needs. Returns the façade, the lead, the worker's
/// bound session, and the lead's input log.
async fn a_lead_and_its_worker() -> (Facade, ProcessId, SessionId, Arc<Mutex<Vec<u8>>>) {
    let (spawner, lead_input) = FakeSpawner::records_input();
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(Path::new("/"), Some("proj"), None)
        .expect("seed a project")
        .id;
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(spawner),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(vec![AgentTool {
            name: "worker".into(),
            command: "true".into(),
            default_args: Vec::new(),
            kind: AgentKind::Generic,
            prompt_mode: PromptMode::AppendedArg,
        }])))
        .build(),
    );
    let mut rx = facade.subscribe();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    facade
        .scoped(lead_session)
        .start_process(lead)
        .expect("start the lead");
    wait_process_to(&mut rx, lead, ProcStatus::Running).await;

    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new(), false)
        .expect("a lead spawns a worker");
    wait_process_to(&mut rx, worker, ProcStatus::Running).await;
    let worker_session = authentic_session(&facade, worker, TEST_PEER_PGID + 1);
    facade
        .scoped(worker_session)
        .bind_session_process(worker)
        .expect("an authentic bind to the worker");
    (facade, worker, worker_session, lead_input)
}

/// Awaits a complete turn reaching the lead's PTY — the write is queued on the process's bounded
/// input channel and applied by its pump, so it lands a scheduling turn after the call returns. A
/// turn is complete once its submitting carriage return has arrived.
async fn wait_for_a_turn(log: &Arc<Mutex<Vec<u8>>>) -> String {
    let arrival = async {
        loop {
            let written = lock(log).clone();
            if written.last() == Some(&b'\r') {
                return String::from_utf8(written).expect("utf-8 input");
            }
            tokio::task::yield_now().await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), arrival)
        .await
        .expect("the report reaches the lead's terminal as a submitted turn")
}

#[tokio::test]
async fn a_workers_report_reaches_its_lead_as_a_submitted_turn() {
    let (facade, worker, worker_session, lead_input) = a_lead_and_its_worker().await;

    facade
        .scoped(worker_session)
        .report_to_lead("the build is green".to_string())
        .expect("a worker reports to its lead");

    let written = wait_for_a_turn(&lead_input).await;
    assert!(
        written.contains("the build is green"),
        "the lead receives the body: {written:?}"
    );
    assert!(
        written.contains(&format!("[Soloist worker #{worker}")),
        "the lead is told which worker reported: {written:?}"
    );
}

#[tokio::test]
async fn a_caller_no_agent_spawned_has_no_lead_to_report_to() {
    // The lead is resolved from lineage, never named by the caller, so a caller with none is
    // refused rather than defaulted onto a terminal — its own or anyone else's. Both shapes of
    // "no lead" are refused: a lead agent, which Soloist knows but nothing spawned, and an
    // external caller it cannot name at all.
    let (facade, project) = facade_with_agent_tool();
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let lead_session = scoped_to(&facade, lead);
    assert!(matches!(
        facade
            .scoped(lead_session)
            .report_to_lead("anything".to_string()),
        Err(ReportToLeadError::NoLead)
    ));

    let external = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade
            .scoped(external)
            .report_to_lead("anything".to_string()),
        Err(ReportToLeadError::NoLead)
    ));
}

#[tokio::test]
async fn a_report_to_a_departed_lead_is_refused() {
    let (facade, _worker, worker_session, _lead_input) = a_lead_and_its_worker().await;
    let lead = facade
        .snapshot()
        .into_iter()
        .find(|view| view.label == "lead")
        .expect("the lead is registered")
        .id;
    facade
        .supervisor()
        .close(lead)
        .await
        .expect("close the lead");

    // The recorded parent outlives the process; the registry is what says whether it is there.
    assert!(matches!(
        facade
            .scoped(worker_session)
            .report_to_lead("too late".to_string()),
        Err(ReportToLeadError::LeadGone)
    ));
}

#[tokio::test]
async fn an_oversized_report_is_refused_and_the_cap_itself_is_allowed() {
    let (facade, _worker, worker_session, lead_input) = a_lead_and_its_worker().await;

    assert!(matches!(
        facade
            .scoped(worker_session)
            .report_to_lead("x".repeat(MAX_REPORT_BYTES + 1)),
        Err(ReportToLeadError::TooLong { max_bytes, .. }) if max_bytes == MAX_REPORT_BYTES
    ));

    // A report exactly at the cap is delivered, so the bound is inclusive rather than off by one
    // — and what reaches the lead is that report, with nothing of the refused one before it.
    facade
        .scoped(worker_session)
        .report_to_lead("y".repeat(MAX_REPORT_BYTES))
        .expect("a report at the cap is delivered");

    let written = wait_for_a_turn(&lead_input).await;
    assert!(
        !written.contains('x'),
        "the refused report never reached the lead"
    );
    assert!(written.contains(&"y".repeat(MAX_REPORT_BYTES)));
}

#[test]
fn bulk_commands_without_a_project_in_scope_are_refused() {
    let (facade, _trust) = facade();
    // A process exists, but the unbound session has no project in scope, so a project-wide
    // bulk action is ambiguous — every bulk entry point refuses it the same way.
    terminal_in(&facade, ProjectId::from_raw(1), "term");
    let session = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade.scoped(session).start_all_commands(),
        Err(ScopedActionError::NoProjectScope)
    ));
    assert!(matches!(
        facade.scoped(session).stop_all_commands(),
        Err(ScopedActionError::NoProjectScope)
    ));
    assert!(matches!(
        facade.scoped(session).restart_all_commands(),
        Err(ScopedActionError::NoProjectScope)
    ));
}

/// Registers a trusted command in `project` and returns its id — a startable command the
/// bulk-scope tests target.
fn trusted_command_in(
    facade: &Facade,
    trust: &FakeTrustRepo,
    project: ProjectId,
    name: &str,
) -> ProcessId {
    let config = parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
    let spec = config.processes.get("Web").cloned().expect("Web");
    let id =
        facade
            .supervisor()
            .register(Registration::command(project, Path::new("/p"), name, &spec));
    trust
        .set_trusted(project, &spec.variant_hash())
        .expect("trust the command");
    id
}

#[test]
fn services_list_returns_only_the_in_scope_projects_commands() {
    let (facade, trust) = facade();
    let command = trusted_command_in(&facade, &trust, ProjectId::from_raw(1), "Web");
    // A terminal in the same project is not a service; a command in another project is out
    // of scope. Neither must appear.
    terminal_in(&facade, ProjectId::from_raw(1), "shell");
    trusted_command_in(&facade, &trust, ProjectId::from_raw(2), "Other");
    let session = scoped_to(&facade, command);

    let services = facade
        .scoped(session)
        .services_list()
        .expect("an in-scope services list");
    let ids: Vec<_> = services.iter().map(|view| view.id).collect();
    assert_eq!(ids, vec![command], "only the in-scope project's commands");

    // Unscoped, the query is ambiguous and refused like the other scoped operations.
    let unscoped = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade.scoped(unscoped).services_list(),
        Err(ScopedActionError::NoProjectScope)
    ));
}

#[tokio::test]
async fn start_all_commands_acts_only_on_the_in_scope_project() {
    let (facade, trust) = facade();
    let mut rx = facade.subscribe();
    let here = trusted_command_in(&facade, &trust, ProjectId::from_raw(1), "Here");
    let elsewhere = trusted_command_in(&facade, &trust, ProjectId::from_raw(2), "Elsewhere");
    // The caller runs in a process in project 1, so binding resolves that project as its scope
    // (the projects registry is empty in this fake, so a process binding is how scope resolves
    // here).
    let session = scoped_to(&facade, here);

    let summary = facade
        .scoped(session)
        .start_all_commands()
        .expect("an in-scope bulk start");
    assert_eq!(
        summary.started,
        vec![here],
        "only the in-scope project's command starts"
    );
    wait_for(&mut rx, ProcStatus::Running).await;
    assert_eq!(
        facade.process_view(elsewhere).expect("registered").status,
        ProcStatus::Stopped,
        "the other project's command is untouched"
    );
}

#[test]
fn clear_output_enforces_scope() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let elsewhere = terminal_in(&facade, ProjectId::from_raw(2), "elsewhere");
    let session = scoped_to(&facade, here);
    // In scope: the action is allowed. The process never started, so there is no terminal
    // to clear, reported as false — but the call is permitted, not refused.
    assert!(
        !facade
            .scoped(session)
            .clear_output(here)
            .expect("an in-scope clear"),
        "a never-started process has no terminal to clear"
    );
    // Out of scope: refused by the shared scope guard, like the other scoped actions.
    assert!(matches!(
        facade.scoped(session).clear_output(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
}

#[tokio::test]
async fn an_untrusted_command_in_scope_is_refused() {
    let (facade, trust) = facade();
    let config = parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
    let spec = config.processes.get("Web").cloned().expect("Web");
    let project = ProjectId::from_raw(1);
    let id = facade.supervisor().register(Registration::command(
        project,
        Path::new("/p"),
        "Web",
        &spec,
    ));
    let session = scoped_to(&facade, id);

    // In scope, but the trust gate in C2 still refuses an untrusted command.
    assert!(matches!(
        facade.scoped(session).start_process(id),
        Err(ScopedActionError::Untrusted)
    ));

    // Once trusted, the same scoped call starts it — proving the guard is not the blocker.
    trust
        .set_trusted(project, &spec.variant_hash())
        .expect("trust the command");
    facade
        .scoped(session)
        .start_process(id)
        .expect("starts once trusted");
}

#[test]
fn rename_process_enforces_scope() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let elsewhere = terminal_in(&facade, ProjectId::from_raw(2), "elsewhere");
    let session = scoped_to(&facade, here);

    // In scope: the relabel lands on the read model (no trust gate — a rename runs nothing).
    facade
        .scoped(session)
        .rename_process(here, "renamed".into())
        .expect("an in-scope rename");
    assert_eq!(
        facade.process_view(here).expect("registered").label,
        "renamed"
    );

    // Out of scope: refused by the shared scope guard, leaving the label untouched.
    assert!(matches!(
        facade.scoped(session).rename_process(elsewhere, "x".into()),
        Err(ScopedActionError::OutOfScope)
    ));
    assert_eq!(
        facade.process_view(elsewhere).expect("registered").label,
        "elsewhere"
    );
}

#[test]
fn read_tools_enforce_scope() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let elsewhere = terminal_in(&facade, ProjectId::from_raw(2), "elsewhere");
    let session = scoped_to(&facade, here);

    // In scope: each read succeeds (an empty result for a never-started process).
    assert!(facade.scoped(session).process_status_scoped(here).is_ok());
    assert!(facade
        .scoped(session)
        .process_output_scoped(here, None)
        .expect("in-scope output")
        .is_empty());
    assert!(facade
        .scoped(session)
        .process_raw_output_scoped(here)
        .expect("in-scope raw output")
        .is_empty());
    assert!(facade
        .scoped(session)
        .search_output_scoped(here, "x", None)
        .expect("in-scope search")
        .is_empty());
    assert!(facade
        .scoped(session)
        .search_raw_output_scoped(here, "x", None)
        .expect("in-scope raw search")
        .is_empty());
    assert!(facade
        .scoped(session)
        .process_ports_scoped(here)
        .expect("in-scope ports")
        .is_empty());

    // Out of scope: every read refuses the cross-project process, so its output — which can
    // hold another project's secrets — never crosses the isolation boundary.
    assert!(matches!(
        facade.scoped(session).process_status_scoped(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade
            .scoped(session)
            .process_output_scoped(elsewhere, None),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade.scoped(session).process_raw_output_scoped(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade
            .scoped(session)
            .search_output_scoped(elsewhere, "x", None),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade
            .scoped(session)
            .search_raw_output_scoped(elsewhere, "x", None),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade.scoped(session).process_ports_scoped(elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
}

#[test]
fn a_scoped_read_refuses_an_unknown_process_and_an_unscoped_session() {
    let (facade, _trust) = facade();
    let id = terminal_in(&facade, ProjectId::from_raw(1), "term");
    let session = scoped_to(&facade, id);
    // An unknown id is refused before scope is even consulted.
    assert!(matches!(
        facade
            .scoped(session)
            .process_output_scoped(ProcessId::from_raw(999), None),
        Err(ScopedActionError::UnknownProcess)
    ));
    // A session with no project in scope cannot read a process — ambiguous, so refused, and
    // it discloses nothing.
    let unscoped = facade.open_session(PeerCredentials::unauthenticated());
    assert!(matches!(
        facade.scoped(unscoped).process_output_scoped(id, None),
        Err(ScopedActionError::NoProjectScope)
    ));
}

#[test]
fn snapshot_scoped_redacts_out_of_scope_rows_to_identity() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    // An untrusted command in another project: its full view flags `requires_trust`, which the
    // scoped snapshot must strip for an out-of-scope caller.
    let config = parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
    let spec = config.processes.get("Web").cloned().expect("Web");
    let elsewhere = facade.supervisor().register(Registration::command(
        ProjectId::from_raw(2),
        Path::new("/p"),
        "Web",
        &spec,
    ));
    let session = scoped_to(&facade, here);

    let rows = facade.scoped(session).snapshot_scoped();
    let in_scope = rows.iter().find(|v| v.id == here).expect("in-scope row");
    let foreign = rows
        .iter()
        .find(|v| v.id == elsewhere)
        .expect("foreign row");

    // The in-scope row is unchanged.
    assert_eq!(in_scope.label, "here");
    assert_eq!(in_scope.kind, ProcessKind::Terminal);
    // The foreign row keeps identity (name, kind, status) but drops the trust flag.
    assert_eq!(foreign.label, "Web", "identity (name) is kept");
    assert_eq!(foreign.kind, ProcessKind::Command);
    assert_eq!(foreign.status, ProcStatus::Stopped);
    assert!(!foreign.requires_trust, "trust state is redacted away");
    // Its full (unscoped) view still carries the flag — proving the snapshot redacted a copy,
    // not the source of truth.
    assert!(
        facade
            .process_view(elsewhere)
            .expect("registered")
            .requires_trust
    );
}

#[tokio::test]
async fn close_process_enforces_scope() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let elsewhere = terminal_in(&facade, ProjectId::from_raw(2), "elsewhere");
    let session = scoped_to(&facade, here);

    // Out of scope: refused before anything is removed.
    assert!(matches!(
        facade.scoped(session).close_process(elsewhere).await,
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(
        facade.process_view(elsewhere).is_some(),
        "a refused close removes nothing"
    );

    // In scope (a resting process): removed from the registry entirely.
    facade
        .scoped(session)
        .close_process(here)
        .await
        .expect("an in-scope close");
    assert!(
        facade.process_view(here).is_none(),
        "an in-scope close forgets the process"
    );
}

#[test]
fn project_processes_scoped_redacts_a_foreign_project_the_caller_names() {
    // Naming another project is allowed — `list_projects` lists them all — but asking for its
    // status must not hand back what `snapshot_scoped` refuses. This is the read the IPC adapter
    // used to compose from an unscoped snapshot, which returned foreign rows in full.
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let config = parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
    let spec = config.processes.get("Web").cloned().expect("Web");
    let foreign_project = ProjectId::from_raw(2);
    let elsewhere = facade.supervisor().register(Registration::command(
        foreign_project,
        Path::new("/p"),
        "Web",
        &spec,
    ));
    let session = scoped_to(&facade, here);

    let rows = facade
        .scoped(session)
        .project_processes_scoped(foreign_project);

    let row = rows
        .iter()
        .find(|v| v.id == elsewhere)
        .expect("foreign row");
    assert_eq!(row.label, "Web", "identity is kept");
    assert!(
        !row.requires_trust,
        "a foreign project's trust state is redacted, as it is in snapshot_scoped"
    );
    assert!(
        facade
            .process_view(elsewhere)
            .expect("registered")
            .requires_trust,
        "the unscoped view still carries it — a copy was redacted, not the source"
    );
}

#[test]
fn project_processes_scoped_returns_the_callers_own_project_in_full() {
    let (facade, _trust) = facade();
    let here = terminal_in(&facade, ProjectId::from_raw(1), "here");
    let session = scoped_to(&facade, here);

    let rows = facade
        .scoped(session)
        .project_processes_scoped(ProjectId::from_raw(1));

    let row = rows.iter().find(|v| v.id == here).expect("in-scope row");
    assert_eq!(row.label, "here");
    assert_eq!(row.kind, ProcessKind::Terminal);
}
