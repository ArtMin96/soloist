use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::coordination::AcquireOutcome;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{CorePorts, ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, terminal_registration, FakeLockRepo, FakeProjectRepo, FakeSpawner,
    FakeTrustRepo, TEST_PEER_PGID,
};

/// A façade over in-memory fakes with the lease store wired, so the coordination methods persist.
fn facade_with(projects: Arc<FakeProjectRepo>) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .lock_repo(Arc::new(FakeLockRepo::new()))
        .build(),
    )
}

/// Binds a session to a fresh terminal in `project`, as the UDS adapter would for an MCP client
/// running inside that process — so the session has both an effective project and a lease owner.
fn bound_session(facade: &Facade, project: ProjectId) -> (SessionId, ProcessId) {
    let id = facade
        .supervisor()
        .register(terminal_registration(project, "term", "sleep 60"));
    let session = authentic_session(facade, id, TEST_PEER_PGID);
    facade
        .bind_session_process(session, id)
        .expect("an authentic bind to the process the caller runs in");
    (session, id)
}

#[test]
fn acquiring_with_no_project_in_scope_is_refused() {
    // No projects loaded and an unbound session → no effective project to scope the lease to.
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let session = facade.open_session(None);

    assert!(matches!(
        facade.lock_acquire(session, "deploy", Some(Duration::from_secs(30))),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn acquiring_without_a_bound_process_is_refused() {
    // One project loaded gives an unbound session the single-project default scope, but with no
    // bound process there is no owner to attribute (and auto-release) the lease to.
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/tmp/soloist-coordination-test"), Some("p"), None)
        .expect("seed one project");
    let facade = facade_with(projects);
    let session = facade.open_session(None);

    assert!(matches!(
        facade.lock_acquire(session, "deploy", Some(Duration::from_secs(30))),
        Err(CoordinationError::NoBoundProcess)
    ));
}

#[test]
fn a_bound_session_acquires_reads_and_releases_a_lease() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let (session, owner) = bound_session(&facade, ProjectId::from_raw(1));

    let outcome = facade
        .lock_acquire(session, "deploy", Some(Duration::from_secs(30)))
        .expect("acquire");
    assert!(
        matches!(outcome, AcquireOutcome::Acquired(ref view) if view.owner == owner),
        "the bound process owns the lease"
    );

    let held = facade.lock_status(session, "deploy").expect("status");
    assert_eq!(held.map(|view| view.owner), Some(owner));

    assert!(
        facade.lock_release(session, "deploy").expect("release"),
        "the owner releases its own lease"
    );
    assert!(
        facade
            .lock_status(session, "deploy")
            .expect("status")
            .is_none(),
        "the key is free after release"
    );
}
