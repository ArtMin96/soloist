use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::coordination::{AcquireOutcome, IdleMode, TimerStatus, MAX_TIMER_BODY_BYTES};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{CorePorts, ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, terminal_registration, FakeLockRepo, FakeProjectRepo, FakeSpawner,
    FakeTimerRepo, FakeTrustRepo, TEST_PEER_PGID,
};

/// A façade over in-memory fakes with the lease and timer stores wired, so the coordination
/// methods persist.
fn facade_with(projects: Arc<FakeProjectRepo>) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .lock_repo(Arc::new(FakeLockRepo::new()))
        .timer_repo(Arc::new(FakeTimerRepo::new()))
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

#[test]
fn setting_a_timer_without_a_bound_process_is_refused() {
    // One project loaded gives an unbound session the single-project default scope, but with no
    // bound process there is no owner to deliver the timer's body to (and clean it up on close).
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/tmp/soloist-timer-test"), Some("p"), None)
        .expect("seed one project");
    let facade = facade_with(projects);
    let session = facade.open_session(None);

    assert!(matches!(
        facade.timer_set(session, "ping".into(), Some(Duration::from_secs(30))),
        Err(CoordinationError::NoBoundProcess)
    ));
}

#[test]
fn a_bound_session_sets_lists_pauses_and_cancels_a_timer() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let (session, _owner) = bound_session(&facade, ProjectId::from_raw(1));

    let view = facade
        .timer_set(session, "resume".into(), Some(Duration::from_secs(30)))
        .expect("set");
    assert_eq!(view.status, TimerStatus::Armed);

    let listed = facade.timer_list(session).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].body, "resume");

    assert!(facade.timer_pause(session, view.id).expect("pause"));
    assert_eq!(
        facade.timer_list(session).expect("list")[0].status,
        TimerStatus::Paused
    );

    assert!(facade.timer_cancel(session, view.id).expect("cancel"));
    assert!(facade.timer_list(session).expect("list").is_empty());
}

#[test]
fn setting_a_timer_with_a_body_over_the_cap_arms_nothing() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let (session, _owner) = bound_session(&facade, ProjectId::from_raw(1));

    let oversized = "x".repeat(MAX_TIMER_BODY_BYTES + 1);
    assert!(matches!(
        facade.timer_set(session, oversized, Some(Duration::from_secs(30))),
        Err(CoordinationError::PayloadTooLarge { .. })
    ));
    assert!(
        facade.timer_list(session).expect("list").is_empty(),
        "a rejected write must arm no timer"
    );
}

#[test]
fn setting_a_timer_with_a_body_at_the_cap_arms_it() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let (session, _owner) = bound_session(&facade, ProjectId::from_raw(1));

    let at_cap = "x".repeat(MAX_TIMER_BODY_BYTES);
    facade
        .timer_set(session, at_cap, Some(Duration::from_secs(30)))
        .expect("a body exactly at the cap is accepted");
    assert_eq!(facade.timer_list(session).expect("list").len(), 1);
}

#[test]
fn fire_when_idle_reports_the_processes_it_is_waiting_on() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let project = ProjectId::from_raw(1);
    let (session, _owner) = bound_session(&facade, project);
    // Two registered processes, running but not classified idle: in the registry with no idle
    // signal, so the timer waits on both.
    let watched = vec![
        facade
            .supervisor()
            .register(terminal_registration(project, "first", "sleep 60")),
        facade
            .supervisor()
            .register(terminal_registration(project, "second", "sleep 60")),
    ];

    let outcome = facade
        .timer_fire_when_idle(
            session,
            "all done".into(),
            watched.clone(),
            IdleMode::All,
            Some(Duration::from_secs(60)),
        )
        .expect("set");

    // Neither is idle, so the condition is not yet met and the report names both as still pending.
    assert!(!outcome.already_idle);
    assert_eq!(outcome.waiting_on, watched);
}

#[test]
fn fire_when_idle_counts_a_process_absent_from_the_registry_as_idle() {
    // A watched worker that has already exited (left the registry) can no longer work, so it counts
    // as idle — exactly the rule the scheduler fires on. The report must not claim the timer is
    // still waiting on it: otherwise an `all` condition that is in fact already met reads as unmet,
    // and a lead believes a finished worker is still busy.
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let (session, _owner) = bound_session(&facade, ProjectId::from_raw(1));
    let gone = ProcessId::from_raw(9999); // never registered → not in the supervisor

    let outcome = facade
        .timer_fire_when_idle(
            session,
            "all done".into(),
            vec![gone],
            IdleMode::All,
            Some(Duration::from_secs(60)),
        )
        .expect("set");

    assert!(
        outcome.already_idle,
        "an absent watched process counts as idle, so an all-timer's condition is already met"
    );
    assert!(
        outcome.waiting_on.is_empty(),
        "the report must not wait on a process that has left the registry"
    );
}
