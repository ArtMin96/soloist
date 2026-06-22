use super::*;
use soloist_core::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, Origin, ProcessId, TokioClock};
use std::sync::Arc;

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

#[test]
fn whoami_routes_to_the_identity_session() {
    let facade = facade();
    let session = facade.open_session();
    match handle_request(&facade, session, IpcRequest::Whoami) {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.session, session);
            assert_eq!(who.bound_process, None);
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[test]
fn register_agent_acks_and_whoami_reflects_the_label() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::RegisterAgent {
                label: "claude-code".into(),
            },
        ),
        Ok(IpcResponse::Acked)
    );
    match handle_request(&facade, session, IpcRequest::Whoami) {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.origin, Origin::External("claude-code".into()));
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[test]
fn list_processes_returns_the_registered_processes() {
    let facade = facade();
    let session = facade.open_session();
    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "term",
        "sleep 60",
    ));
    match handle_request(&facade, session, IpcRequest::ListProcesses) {
        Ok(IpcResponse::Processes(processes)) => {
            assert_eq!(processes.len(), 1);
            assert_eq!(processes[0].id, id);
        }
        other => panic!("expected the process list, got {other:?}"),
    }
}

#[test]
fn get_process_status_returns_a_registered_process() {
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
    ) {
        Ok(IpcResponse::Process(view)) => assert_eq!(view.id, id),
        other => panic!("expected one process, got {other:?}"),
    }
}

#[test]
fn get_process_status_reports_unknown_for_a_missing_id() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProcessStatus {
                process: ProcessId::from_raw(999),
            },
        ),
        Err(IpcError::UnknownProcess)
    );
}

#[test]
fn list_projects_is_empty_without_any_loaded() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ListProjects),
        Ok(IpcResponse::Projects(Vec::new()))
    );
}

#[test]
fn project_status_without_scope_is_refused() {
    let facade = facade();
    let session = facade.open_session();
    // No project loaded, bound, or selected: an unscoped status request is ambiguous.
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProjectStatus { project: None }
        ),
        Err(IpcError::NoProjectScope)
    );
}

#[test]
fn project_status_for_an_unknown_project_is_refused() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProjectStatus {
                project: Some(ProjectId::from_raw(404)),
            },
        ),
        Err(IpcError::UnknownProject)
    );
}

#[test]
fn binding_an_unknown_process_maps_to_the_wire_error() {
    let facade = facade();
    let session = facade.open_session();
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::BindSessionProcess {
                process: ProcessId::from_raw(7),
            },
        ),
        Err(IpcError::UnknownProcess)
    );
}
