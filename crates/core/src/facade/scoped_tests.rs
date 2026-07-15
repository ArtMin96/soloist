use super::*;
use crate::composition::CorePorts;
use crate::config::parse;
use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::{Clock, TokioClock, TrustRepo};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::Registration;
use crate::sync::lock;
use crate::testing::{
    agent_registration, authentic_session, facade_with_agent_tool, terminal_registration,
    FakeProjectRepo, FakeSpawner, FakeTrustRepo, TEST_PEER_PGID,
};
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
    let session = facade.open_session(None);
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
    let session = facade.open_session(None);
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
    let session = facade.open_session(None);
    assert!(matches!(
        facade.scoped(session).spawn_agent("Claude", Vec::new()),
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
        facade.scoped(session).spawn_agent("NoSuchTool", Vec::new()),
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
        .spawn_agent("worker", Vec::new())
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
            .spawn_agent("worker", Vec::new()),
        Err(SpawnAgentError::WorkerMayNotSpawn)
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
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
        .spawn_agent("worker", Vec::new())
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
            .spawn_agent("worker", Vec::new()),
        Err(SpawnAgentError::WorkerMayNotSpawn)
    ));
}

#[test]
fn bulk_commands_without_a_project_in_scope_are_refused() {
    let (facade, _trust) = facade();
    // A process exists, but the unbound session has no project in scope, so a project-wide
    // bulk action is ambiguous — every bulk entry point refuses it the same way.
    terminal_in(&facade, ProjectId::from_raw(1), "term");
    let session = facade.open_session(None);
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
    let unscoped = facade.open_session(None);
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
    let unscoped = facade.open_session(None);
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
