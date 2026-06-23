use super::*;
use crate::config::parse;
use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::{Clock, CorePorts, TokioClock, TrustRepo};
use crate::process::ProcStatus;
use crate::supervisor::Registration;
use crate::sync::lock;
use crate::testing::{
    authentic_session, terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo,
    TEST_PEER_PGID,
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
        .bind_session_process(session, process)
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
        .start_process(session, id)
        .expect("an in-scope terminal starts");
    wait_for(&mut rx, ProcStatus::Running).await;

    assert!(
        facade.stop_process(session, id).expect("in-scope stop"),
        "a running process reports it was live"
    );
    wait_for(&mut rx, ProcStatus::Stopped).await;
}

#[test]
fn an_unknown_process_is_refused() {
    let (facade, _trust) = facade();
    let session = facade.open_session(None);
    assert!(matches!(
        facade.start_process(session, ProcessId::from_raw(999)),
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
        facade.start_process(session, id),
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
        facade.start_process(session, elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade.stop_process(session, elsewhere),
        Err(ScopedActionError::OutOfScope)
    ));
    assert!(matches!(
        facade.restart_process(session, elsewhere),
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
        .start_process(session, id)
        .expect("an in-scope start");
    wait_for(&mut rx, ProcStatus::Running).await;

    // A wait far beyond the cap is clamped to MAX_INPUT_WAIT before the clock ever sleeps, so a
    // remote caller cannot tie up the request (and the connection behind it) with a huge value.
    facade
        .send_input(session, id, b"x".to_vec(), Some(Duration::from_secs(3600)))
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
            .send_input(session, elsewhere, b"x".to_vec(), None)
            .await,
        Err(ScopedActionError::OutOfScope)
    ));
}

#[test]
fn spawn_agent_without_a_project_in_scope_is_refused() {
    let (facade, _trust) = facade();
    let session = facade.open_session(None);
    assert!(matches!(
        facade.spawn_agent(session, "Claude", Vec::new()),
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
        facade.spawn_agent(session, "NoSuchTool", Vec::new()),
        Err(SpawnAgentError::Launch(LaunchAgentError::UnknownTool))
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
        facade.start_all_commands(session),
        Err(ScopedActionError::NoProjectScope)
    ));
    assert!(matches!(
        facade.stop_all_commands(session),
        Err(ScopedActionError::NoProjectScope)
    ));
    assert!(matches!(
        facade.restart_all_commands(session),
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
        .services_list(session)
        .expect("an in-scope services list");
    let ids: Vec<_> = services.iter().map(|view| view.id).collect();
    assert_eq!(ids, vec![command], "only the in-scope project's commands");

    // Unscoped, the query is ambiguous and refused like the other scoped operations.
    let unscoped = facade.open_session(None);
    assert!(matches!(
        facade.services_list(unscoped),
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
        .start_all_commands(session)
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
            .clear_output(session, here)
            .expect("an in-scope clear"),
        "a never-started process has no terminal to clear"
    );
    // Out of scope: refused by the shared scope guard, like the other scoped actions.
    assert!(matches!(
        facade.clear_output(session, elsewhere),
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
        facade.start_process(session, id),
        Err(ScopedActionError::Untrusted)
    ));

    // Once trusted, the same scoped call starts it — proving the guard is not the blocker.
    trust
        .set_trusted(project, &spec.variant_hash())
        .expect("trust the command");
    facade
        .start_process(session, id)
        .expect("starts once trusted");
}
