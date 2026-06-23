use super::*;
use soloist_core::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, DomainEvent, Origin, ProcStatus, ProcessId, TokioClock};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// A façade over in-memory fakes — an alternate composition root, the same way the core's
/// own tests build one. Routing is what we exercise here; the behaviour behind each call
/// is tested in the core.
fn facade() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    )
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
async fn whoami_routes_to_the_identity_session() {
    let facade = facade();
    let session = facade.open_session();
    match handle_request(&facade, session, IpcRequest::Whoami).await {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.session, session);
            assert_eq!(who.bound_process, None);
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[tokio::test]
async fn register_agent_acks_and_whoami_reflects_the_label() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::RegisterAgent {
                label: "claude-code".into(),
            },
        )
        .await,
        Ok(IpcResponse::Acked)
    );
    match handle_request(&facade, session, IpcRequest::Whoami).await {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.origin, Origin::External("claude-code".into()));
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[tokio::test]
async fn list_processes_returns_the_registered_processes() {
    let facade = facade();
    let session = facade.open_session();
    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "term",
        "sleep 60",
    ));
    match handle_request(&facade, session, IpcRequest::ListProcesses).await {
        Ok(IpcResponse::Processes(processes)) => {
            assert_eq!(processes.len(), 1);
            assert_eq!(processes[0].id, id);
        }
        other => panic!("expected the process list, got {other:?}"),
    }
}

#[tokio::test]
async fn get_process_status_returns_a_registered_process() {
    let facade = facade();
    let session = facade.open_session();
    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "term",
        "sleep 60",
    ));
    match handle_request(
        &facade,
        session,
        IpcRequest::GetProcessStatus { process: id },
    )
    .await
    {
        Ok(IpcResponse::Process(view)) => assert_eq!(view.id, id),
        other => panic!("expected one process, got {other:?}"),
    }
}

#[tokio::test]
async fn get_process_status_reports_unknown_for_a_missing_id() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProcessStatus {
                process: ProcessId::from_raw(999),
            },
        )
        .await,
        Err(IpcError::UnknownProcess)
    );
}

#[tokio::test]
async fn list_projects_is_empty_without_any_loaded() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ListProjects).await,
        Ok(IpcResponse::Projects(Vec::new()))
    );
}

#[tokio::test]
async fn project_status_without_scope_is_refused() {
    let facade = facade();
    let session = facade.open_session();
    // No project loaded, bound, or selected: an unscoped status request is ambiguous.
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProjectStatus { project: None }
        )
        .await,
        Err(IpcError::NoProjectScope)
    );
}

#[tokio::test]
async fn project_status_for_an_unknown_project_is_refused() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProjectStatus {
                project: Some(ProjectId::from_raw(404)),
            },
        )
        .await,
        Err(IpcError::UnknownProject)
    );
}

#[tokio::test]
async fn binding_an_unknown_process_maps_to_the_wire_error() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::BindSessionProcess {
                process: ProcessId::from_raw(7),
            },
        )
        .await,
        Err(IpcError::UnknownProcess)
    );
}

/// Registers a terminal in `project` and binds `session` to it, putting that project in
/// scope — the setup every action-routing test shares.
fn scoped_terminal(
    facade: &Facade,
    session: SessionId,
    project: ProjectId,
    name: &str,
) -> ProcessId {
    let id = facade
        .supervisor()
        .register(terminal_registration(project, name, "sleep 60"));
    facade
        .bind_session_process(session, id)
        .expect("bind to the registered process");
    id
}

#[tokio::test]
async fn starting_an_in_scope_process_is_acked() {
    let facade = facade();
    let session = facade.open_session();
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StartProcess { process: id }).await,
        Ok(IpcResponse::Acked)
    );
}

#[tokio::test]
async fn stopping_an_idle_in_scope_process_reports_it_was_not_running() {
    let facade = facade();
    let session = facade.open_session();
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    // Never started, so the stop finds nothing live — the bool the agent reads back.
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StopProcess { process: id }).await,
        Ok(IpcResponse::Stopped(false))
    );
}

#[tokio::test]
async fn sending_input_without_a_wait_returns_no_tail() {
    let facade = facade();
    let mut rx = facade.subscribe();
    let session = facade.open_session();
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    facade.supervisor().start(id).expect("terminal starts");
    wait_for(&mut rx, ProcStatus::Running).await;

    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::SendInput {
                process: id,
                input: "ls\r".into(),
                wait_ms: None,
            },
        )
        .await,
        Ok(IpcResponse::InputSent(None))
    );
}

#[tokio::test]
async fn an_action_on_another_projects_process_maps_to_out_of_scope() {
    let facade = facade();
    let session = facade.open_session();
    // The session is scoped to project 1; the target lives in project 2.
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "here");
    let elsewhere = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(2),
        "elsewhere",
        "sleep 60",
    ));
    for request in [
        IpcRequest::StartProcess { process: elsewhere },
        IpcRequest::StopProcess { process: elsewhere },
        IpcRequest::RestartProcess { process: elsewhere },
        IpcRequest::SendInput {
            process: elsewhere,
            input: "x".into(),
            wait_ms: None,
        },
    ] {
        assert_eq!(
            handle_request(&facade, session, request).await,
            Err(IpcError::OutOfScope)
        );
    }
}
