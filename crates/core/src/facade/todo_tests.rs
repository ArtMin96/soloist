use crate::facade::Facade;
use crate::ids::SessionId;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::TodoStatus;
use crate::ids::ProjectId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, terminal_registration, FakeProjectRepo, FakeSpawner, FakeTodoRepo,
    FakeTrustRepo, TEST_PEER_PGID,
};

fn doc(title: &str, status: TodoStatus) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: "do it".into(),
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
        facade
            .scoped(session)
            .todo_create(doc("x", TodoStatus::Open)),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_session_creates_lists_and_completes_without_binding_a_process() {
    let (facade, session) = scoped_facade();

    let created = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open))
        .expect("create with only project scope");
    let listed = facade.scoped(session).todo_list().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let done = facade
        .scoped(session)
        .todo_complete(created.id)
        .expect("complete");
    assert_eq!(done.doc.status, TodoStatus::Done);
}

#[test]
fn a_stale_update_surfaces_a_todo_revision_conflict() {
    let (facade, session) = scoped_facade();
    let todo = facade
        .scoped(session)
        .todo_create(doc("v1", TodoStatus::Open))
        .expect("create");
    facade
        .scoped(session)
        .todo_update(todo.id, doc("v2", TodoStatus::InProgress), 1)
        .expect("first update");

    assert!(matches!(
        facade
            .scoped(session)
            .todo_update(todo.id, doc("v3", TodoStatus::InProgress), 1),
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
    bad.title = "  ".into();
    assert!(matches!(
        facade.scoped(session).todo_create(bad),
        Err(CoordinationError::InvalidTodo(_))
    ));
}

#[test]
fn completing_a_blocked_todo_surfaces_todo_blocked() {
    let (facade, session) = scoped_facade();
    let blocker = facade
        .scoped(session)
        .todo_create(doc("dep", TodoStatus::Open))
        .expect("create blocker");
    let gated = facade
        .scoped(session)
        .todo_create(doc("main", TodoStatus::Open))
        .expect("create dependent");
    facade
        .scoped(session)
        .todo_set_blockers(gated.id, vec![blocker.id])
        .expect("set blocker");

    assert!(matches!(
        facade.scoped(session).todo_complete(gated.id),
        Err(CoordinationError::TodoBlocked { by }) if by == vec![blocker.id]
    ));
}

#[test]
fn an_unknown_todo_is_reported() {
    let (facade, session) = scoped_facade();
    assert!(matches!(
        facade
            .scoped(session)
            .todo_get(crate::ids::TodoId::from_raw(404)),
        Err(CoordinationError::UnknownTodo)
    ));
}

#[test]
fn locking_a_todo_without_a_bound_process_is_refused() {
    // Content ops work with only project scope, but a lock is process-owned, so it needs a bound
    // process (the owner the supervisor auto-releases it for on close).
    let (facade, session) = scoped_facade();
    let todo = facade
        .scoped(session)
        .todo_create(doc("x", TodoStatus::Open))
        .expect("create");
    assert!(matches!(
        facade.scoped(session).todo_lock(todo.id),
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
        .scoped(session)
        .bind_session_process(owner)
        .expect("authentic bind");

    let todo = facade
        .scoped(session)
        .todo_create(doc("x", TodoStatus::Open))
        .expect("create");
    let locked = facade.scoped(session).todo_lock(todo.id).expect("lock");
    assert_eq!(locked.locked_by, Some(owner));

    let unlocked = facade.scoped(session).todo_unlock(todo.id).expect("unlock");
    assert_eq!(unlocked.locked_by, None);
}

#[test]
fn comments_round_trip_and_report_unknown_targets() {
    let (facade, session) = scoped_facade();
    let todo = facade
        .scoped(session)
        .todo_create(doc("x", TodoStatus::Open))
        .expect("create");

    let (_, comment) = facade
        .scoped(session)
        .todo_comment_create(todo.id, "note")
        .expect("comment");
    facade
        .scoped(session)
        .todo_comment_update(todo.id, comment, "edited")
        .expect("update comment");
    let listed = facade
        .scoped(session)
        .todo_comment_list(todo.id)
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].body, "edited");

    assert!(matches!(
        facade.scoped(session).todo_comment_delete(todo.id, 999),
        Err(CoordinationError::UnknownComment)
    ));
}

#[test]
fn an_unbound_callers_comment_is_unattributed() {
    // `scoped_facade` is an unbound single-project session: it can comment, but the core stamps no
    // author — there is no bound actor and the caller cannot supply one (no spoofing).
    let (facade, session) = scoped_facade();
    let todo = facade
        .scoped(session)
        .todo_create(doc("x", TodoStatus::Open))
        .expect("create");
    let (view, _) = facade
        .scoped(session)
        .todo_comment_create(todo.id, "drive-by note")
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
        .scoped(session)
        .bind_session_process(owner)
        .expect("authentic bind");

    let todo = facade
        .scoped(session)
        .todo_create(doc("x", TodoStatus::Open))
        .expect("create");
    let (view, _) = facade
        .scoped(session)
        .todo_comment_create(todo.id, "reviewed")
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

#[test]
fn todo_transfer_in_preserves_the_document_comments_and_clears_blockers_and_lock() {
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-a"), Some("a"), None)
        .expect("A")
        .id;
    let b = projects
        .upsert(Path::new("/tmp/soloist-b"), Some("b"), None)
        .expect("B")
        .id;
    let facade = facade_with(projects);

    // A todo in A with a comment, a blocker, and a lock (set via a session bound to a process in A).
    let owner = facade
        .supervisor()
        .register(terminal_registration(a, "w", "sleep 1"));
    let session = authentic_session(&facade, owner, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(owner)
        .expect("bind the session to its process in A");
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::InProgress))
        .expect("create");
    let blocker = facade
        .scoped(session)
        .todo_create(doc("dep", TodoStatus::Open))
        .expect("blocker");
    facade
        .scoped(session)
        .todo_add_blocker(todo.id, blocker.id)
        .expect("block");
    facade
        .scoped(session)
        .todo_comment_create(todo.id, "note")
        .expect("comment");
    let locked = facade.scoped(session).todo_lock(todo.id).expect("lock");
    assert_eq!(locked.locked_by, Some(owner));

    // Move it to B via the local/trusted path.
    let moved = facade.todo_transfer_in(a, b, todo.id).expect("transfer");
    assert_eq!(moved.id, todo.id, "the durable id is stable");
    assert_eq!(moved.revision, todo.revision, "the revision is preserved");
    assert_eq!(
        moved.doc.status,
        TodoStatus::InProgress,
        "the document (including status) is preserved"
    );
    assert_eq!(moved.comments.len(), 1, "comments are preserved");
    assert!(
        moved.blockers.is_empty(),
        "blockers reference the source project and are cleared"
    );
    assert_eq!(moved.locked_by, None, "the process-owned lock is cleared");
}

#[test]
fn todo_transfer_refuses_a_target_outside_the_callers_authenticated_scope() {
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-a"), Some("a"), None)
        .expect("A")
        .id;
    let b = projects
        .upsert(Path::new("/tmp/soloist-b"), Some("b"), None)
        .expect("B")
        .id;
    let facade = facade_with(projects);
    // The session authenticates to A (a process it runs in), never B.
    let owner = facade
        .supervisor()
        .register(terminal_registration(a, "w", "sleep 1"));
    let session = authentic_session(&facade, owner, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(owner)
        .expect("bind the session to its process in A");
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open))
        .expect("create in A");

    // Transferring to B — which the caller does not run in — is refused (the cross-scope guard).
    assert!(matches!(
        facade.scoped(session).todo_transfer(b, todo.id),
        Err(CoordinationError::ForeignProject)
    ));
}

#[test]
fn todo_transfer_in_refuses_an_unknown_target_project() {
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-a"), Some("a"), None)
        .expect("A")
        .id;
    let facade = facade_with(projects);
    let session = facade.open_session(None);
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open))
        .expect("create in A");

    // A target project that is not loaded is refused before any move, so a bad id never orphans
    // the todo — it stays readable in A.
    assert!(matches!(
        facade.todo_transfer_in(a, ProjectId::from_raw(9999), todo.id),
        Err(CoordinationError::UnknownProject)
    ));
    assert!(
        facade.scoped(session).todo_get(todo.id).is_ok(),
        "still in A"
    );
}
