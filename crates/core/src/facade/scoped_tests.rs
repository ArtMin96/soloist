use super::*;
use crate::config::parse;
use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::{CorePorts, TokioClock, TrustRepo};
use crate::process::ProcStatus;
use crate::supervisor::Registration;
use crate::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use std::path::Path;
use std::sync::Arc;
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
    let session = facade.open_session();
    // Binding to the process puts its project in scope, the same way a Soloist-launched
    // agent's session resolves its project from the process it runs in.
    facade
        .bind_session_process(session, id)
        .expect("bind to the registered process");

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
    let session = facade.open_session();
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
    let session = facade.open_session();
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
    let session = facade.open_session();
    facade
        .bind_session_process(session, here)
        .expect("scope to project 1");

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
    let session = facade.open_session();
    facade
        .bind_session_process(session, id)
        .expect("scope to the command's project");

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
