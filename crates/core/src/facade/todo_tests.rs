use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::coordination::TodoStatus;
use crate::ids::ProjectId;
use crate::ports::{CorePorts, ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, terminal_registration, FakeProjectRepo, FakeSpawner, FakeTodoRepo,
    FakeTrustRepo, TEST_PEER_PGID,
};

fn doc(title: &str, status: TodoStatus) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        description: "do it".into(),
        acceptance_criteria: vec!["done".into()],
        risks: vec!["none identified".into()],
        status,
    }
}

/// A façade over in-memory fakes with `projects` loaded and the todo store wired.
fn facade_with(projects: Arc<FakeProjectRepo>) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .todo_repo(Arc::new(FakeTodoRepo::new()))
        .build(),
    )
}

/// A façade with one project loaded and an unbound session — the single-project default scope, so
/// todo *content* ops (which need no bound owner) work without binding a process.
fn scoped_facade() -> (Facade, SessionId) {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/tmp/soloist-todo-test"), Some("p"), None)
        .expect("seed one project");
    let facade = facade_with(projects);
    let session = facade.open_session(None);
    (facade, session)
}

#[test]
fn creating_with_no_project_in_scope_is_refused() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let session = facade.open_session(None);
    assert!(matches!(
        facade.todo_create(session, doc("x", TodoStatus::Open)),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_session_creates_lists_and_completes_without_binding_a_process() {
    let (facade, session) = scoped_facade();

    let created = facade
        .todo_create(session, doc("ship", TodoStatus::Open))
        .expect("create with only project scope");
    let listed = facade.todo_list(session).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let done = facade.todo_complete(session, created.id).expect("complete");
    assert_eq!(done.doc.status, TodoStatus::Done);
}

#[test]
fn a_stale_update_surfaces_a_todo_revision_conflict() {
    let (facade, session) = scoped_facade();
    let todo = facade
        .todo_create(session, doc("v1", TodoStatus::Open))
        .expect("create");
    facade
        .todo_update(session, todo.id, doc("v2", TodoStatus::InProgress), 1)
        .expect("first update");

    assert!(matches!(
        facade.todo_update(session, todo.id, doc("v3", TodoStatus::InProgress), 1),
        Err(CoordinationError::TodoRevisionConflict {
            expected: Some(1),
            actual: Some(2)
        })
    ));
}

#[test]
fn a_malformed_create_surfaces_an_invalid_todo() {
    let (facade, session) = scoped_facade();
    let mut bad = doc("x", TodoStatus::Open);
    bad.description = "  ".into();
    assert!(matches!(
        facade.todo_create(session, bad),
        Err(CoordinationError::InvalidTodo(_))
    ));
}

#[test]
fn completing_a_blocked_todo_surfaces_todo_blocked() {
    let (facade, session) = scoped_facade();
    let blocker = facade
        .todo_create(session, doc("dep", TodoStatus::Open))
        .expect("create blocker");
    let gated = facade
        .todo_create(session, doc("main", TodoStatus::Open))
        .expect("create dependent");
    facade
        .todo_set_blockers(session, gated.id, vec![blocker.id])
        .expect("set blocker");

    assert!(matches!(
        facade.todo_complete(session, gated.id),
        Err(CoordinationError::TodoBlocked { by }) if by == vec![blocker.id]
    ));
}

#[test]
fn an_unknown_todo_is_reported() {
    let (facade, session) = scoped_facade();
    assert!(matches!(
        facade.todo_get(session, crate::ids::TodoId::from_raw(404)),
        Err(CoordinationError::UnknownTodo)
    ));
}

#[test]
fn locking_a_todo_without_a_bound_process_is_refused() {
    // Content ops work with only project scope, but a lock is process-owned, so it needs a bound
    // process (the owner the supervisor auto-releases it for on close).
    let (facade, session) = scoped_facade();
    let todo = facade
        .todo_create(session, doc("x", TodoStatus::Open))
        .expect("create");
    assert!(matches!(
        facade.todo_lock(session, todo.id),
        Err(CoordinationError::NoBoundProcess)
    ));
}

#[test]
fn a_bound_session_locks_and_unlocks_a_todo() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let project = ProjectId::from_raw(1);
    let owner = facade
        .supervisor()
        .register(terminal_registration(project, "term", "sleep 60"));
    let session = authentic_session(&facade, owner, TEST_PEER_PGID);
    facade
        .bind_session_process(session, owner)
        .expect("authentic bind");

    let todo = facade
        .todo_create(session, doc("x", TodoStatus::Open))
        .expect("create");
    let locked = facade.todo_lock(session, todo.id).expect("lock");
    assert_eq!(locked.locked_by, Some(owner));

    let unlocked = facade.todo_unlock(session, todo.id).expect("unlock");
    assert_eq!(unlocked.locked_by, None);
}

#[test]
fn comments_round_trip_and_report_unknown_targets() {
    let (facade, session) = scoped_facade();
    let todo = facade
        .todo_create(session, doc("x", TodoStatus::Open))
        .expect("create");

    let (_, comment) = facade
        .todo_comment_create(session, todo.id, "note")
        .expect("comment");
    facade
        .todo_comment_update(session, todo.id, comment, "edited")
        .expect("update comment");
    let listed = facade.todo_comment_list(session, todo.id).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].body, "edited");

    assert!(matches!(
        facade.todo_comment_delete(session, todo.id, 999),
        Err(CoordinationError::UnknownComment)
    ));
}

#[test]
fn an_unbound_callers_comment_is_unattributed() {
    // `scoped_facade` is an unbound single-project session: it can comment, but the core stamps no
    // author — there is no bound actor and the caller cannot supply one (no spoofing).
    let (facade, session) = scoped_facade();
    let todo = facade
        .todo_create(session, doc("x", TodoStatus::Open))
        .expect("create");
    let (view, _) = facade
        .todo_comment_create(session, todo.id, "drive-by note")
        .expect("comment");
    assert_eq!(view.comments[0].author, None);
}

#[test]
fn a_bound_process_stamps_its_actor_on_a_comment() {
    let facade = facade_with(Arc::new(FakeProjectRepo::new()));
    let project = ProjectId::from_raw(1);
    let owner = facade
        .supervisor()
        .register(terminal_registration(project, "term", "sleep 60"));
    let session = authentic_session(&facade, owner, TEST_PEER_PGID);
    facade
        .bind_session_process(session, owner)
        .expect("authentic bind");

    let todo = facade
        .todo_create(session, doc("x", TodoStatus::Open))
        .expect("create");
    let (view, _) = facade
        .todo_comment_create(session, todo.id, "reviewed")
        .expect("comment");
    // The core resolves the author from the bound identity — its id and the process's label — so a
    // caller can never forge a different author (the API takes no author argument at all).
    assert_eq!(
        view.comments[0].author,
        Some(CommentAuthor::Process {
            id: owner,
            label: "term".into(),
        })
    );
}
