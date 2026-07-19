use crate::facade::Facade;
use crate::ids::SessionId;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{CommentAuthor, TodoStatus};
use crate::ids::ProjectId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, terminal_registration, FakeProjectRepo, FakeScratchpadRepo, FakeSpawner,
    FakeTodoRepo, FakeTrustRepo, TEST_PEER_PGID,
};

fn doc(title: &str, status: TodoStatus) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: "do it".into(),
        status,
    }
}

/// A façade over in-memory fakes with `projects` loaded and the todo and scratchpad stores wired,
/// the todo store resolving associations against the scratchpad one exactly as the durable adapter
/// joins them.
fn facade_with(projects: Arc<FakeProjectRepo>) -> Facade {
    let scratchpads = Arc::new(FakeScratchpadRepo::new());
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .todo_repo(Arc::new(FakeTodoRepo::joined(Arc::clone(&scratchpads))))
        .scratchpad_repo(scratchpads)
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
            .todo_create(doc("x", TodoStatus::Open), None),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_session_creates_lists_and_completes_without_binding_a_process() {
    let (facade, session) = scoped_facade();

    let created = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), None)
        .expect("create with only project scope")
        .view;
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
        .todo_create(doc("v1", TodoStatus::Open), None)
        .expect("create")
        .view;
    facade
        .scoped(session)
        .todo_update(
            todo.id,
            doc("v2", TodoStatus::InProgress),
            ScratchpadLink::Unchanged,
            1,
        )
        .expect("first update");

    assert!(matches!(
        facade.scoped(session).todo_update(
            todo.id,
            doc("v3", TodoStatus::InProgress),
            ScratchpadLink::Unchanged,
            1
        ),
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
        facade.scoped(session).todo_create(bad, None),
        Err(CoordinationError::InvalidTodo(_))
    ));
}

#[test]
fn completing_a_blocked_todo_surfaces_todo_blocked() {
    let (facade, session) = scoped_facade();
    let blocker = facade
        .scoped(session)
        .todo_create(doc("dep", TodoStatus::Open), None)
        .expect("create blocker")
        .view;
    let gated = facade
        .scoped(session)
        .todo_create(doc("main", TodoStatus::Open), None)
        .expect("create dependent")
        .view;
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
        .todo_create(doc("x", TodoStatus::Open), None)
        .expect("create")
        .view;
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
        .todo_create(doc("x", TodoStatus::Open), None)
        .expect("create")
        .view;
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
        .todo_create(doc("x", TodoStatus::Open), None)
        .expect("create")
        .view;

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
        .todo_create(doc("x", TodoStatus::Open), None)
        .expect("create")
        .view;
    let (view, _) = facade
        .scoped(session)
        .todo_comment_create(todo.id, "drive-by note")
        .expect("comment");
    assert_eq!(view.comments[0].author, None);
}

#[test]
fn the_local_comment_path_creates_an_unattributed_comment() {
    // The local UI drives no session, so `todo_comment_create_in` posts through the core with no
    // author — the honest in-model behavior for the unbound local user (never a forged label).
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(Path::new("/tmp/soloist-local-comment"), Some("p"), None)
        .expect("seed one project")
        .id;
    let facade = facade_with(projects);
    let session = facade.open_session(None);
    let todo = facade
        .scoped(session)
        .todo_create(doc("x", TodoStatus::Open), None)
        .expect("create")
        .view;

    let view = facade
        .todo_comment_create_in(project, todo.id, "local note")
        .expect("local comment");
    assert_eq!(view.comments.len(), 1);
    assert_eq!(view.comments[0].body, "local note");
    assert_eq!(view.comments[0].author, None);

    assert!(matches!(
        facade.todo_comment_create_in(project, crate::ids::TodoId::from_raw(404), "x"),
        Err(CoordinationError::UnknownTodo)
    ));
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
        .todo_create(doc("x", TodoStatus::Open), None)
        .expect("create")
        .view;
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
        .todo_create(doc("ship", TodoStatus::InProgress), None)
        .expect("create")
        .view;
    let blocker = facade
        .scoped(session)
        .todo_create(doc("dep", TodoStatus::Open), None)
        .expect("blocker")
        .view;
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
        .todo_create(doc("ship", TodoStatus::Open), None)
        .expect("create in A")
        .view;

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
        .todo_create(doc("ship", TodoStatus::Open), None)
        .expect("create in A")
        .view;

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

#[test]
fn a_create_links_the_named_scratchpad_and_reads_its_handle_back() {
    let (facade, session) = scoped_facade();
    let project = ProjectId::from_raw(1);
    let pad = facade
        .scratchpad_write_in(project, "release-plan", "the plan".into(), None)
        .expect("write the scratchpad");

    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), Some("release-plan".into()))
        .expect("create linked to the plan it came from")
        .view;

    let linked = todo.scratchpad.expect("the todo names its scratchpad");
    assert_eq!(linked.id, pad.id);
    assert_eq!(linked.name, "release-plan");
    // The listing carries it too, so the board can group without fetching every document.
    let listed = facade.scoped(session).todo_list().expect("list");
    assert_eq!(
        listed[0].scratchpad.as_ref().map(|link| link.id),
        Some(pad.id)
    );
}

#[test]
fn an_update_that_omits_the_scratchpad_leaves_an_existing_link_standing() {
    // The link is live coordination state beside tags and blockers, not part of the document this
    // call replaces, so a routine title/status edit must never silently destroy it.
    let (facade, session) = scoped_facade();
    let project = ProjectId::from_raw(1);
    facade
        .scratchpad_write_in(project, "release-plan", "the plan".into(), None)
        .expect("write the scratchpad");
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), Some("release-plan".into()))
        .expect("create linked")
        .view;

    let updated = facade
        .scoped(session)
        .todo_update(
            todo.id,
            doc("ship it", TodoStatus::InProgress),
            ScratchpadLink::Unchanged,
            todo.revision,
        )
        .expect("update saying nothing about the association");

    assert_eq!(updated.doc.title, "ship it");
    assert_eq!(
        updated.scratchpad.map(|link| link.name),
        Some("release-plan".to_owned())
    );
}

#[test]
fn an_explicitly_cleared_link_unlinks_the_todo() {
    let (facade, session) = scoped_facade();
    let project = ProjectId::from_raw(1);
    facade
        .scratchpad_write_in(project, "release-plan", "the plan".into(), None)
        .expect("write the scratchpad");
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), Some("release-plan".into()))
        .expect("create linked")
        .view;

    let updated = facade
        .scoped(session)
        .todo_update(
            todo.id,
            doc("ship", TodoStatus::Open),
            ScratchpadLink::Cleared,
            todo.revision,
        )
        .expect("update clearing the association");

    assert_eq!(updated.scratchpad, None);
}

#[test]
fn a_relink_moves_the_todo_to_another_scratchpad() {
    let (facade, session) = scoped_facade();
    let project = ProjectId::from_raw(1);
    facade
        .scratchpad_write_in(project, "release-plan", "the plan".into(), None)
        .expect("write the first scratchpad");
    let other = facade
        .scratchpad_write_in(project, "rollout-plan", "the other plan".into(), None)
        .expect("write the second scratchpad");
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), Some("release-plan".into()))
        .expect("create linked")
        .view;

    let updated = facade
        .scoped(session)
        .todo_update(
            todo.id,
            doc("ship", TodoStatus::Open),
            ScratchpadLink::Linked("rollout-plan".into()),
            todo.revision,
        )
        .expect("update relinking the association");

    assert_eq!(updated.scratchpad.map(|link| link.id), Some(other.id));
}

#[test]
fn creating_with_an_unknown_scratchpad_name_is_refused_and_writes_nothing() {
    let (facade, session) = scoped_facade();

    let refused = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), Some("no-such-plan".into()));

    assert!(matches!(refused, Err(CoordinationError::UnknownScratchpad)));
    assert!(
        facade.scoped(session).todo_list().expect("list").is_empty(),
        "a refused create must leave no todo behind"
    );
}

#[test]
fn updating_with_an_unknown_scratchpad_name_is_refused_and_writes_nothing() {
    let (facade, session) = scoped_facade();
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), None)
        .expect("create")
        .view;

    let refused = facade.scoped(session).todo_update(
        todo.id,
        doc("renamed", TodoStatus::Done),
        ScratchpadLink::Linked("no-such-plan".into()),
        todo.revision,
    );

    assert!(matches!(refused, Err(CoordinationError::UnknownScratchpad)));
    let unchanged = facade.scoped(session).todo_get(todo.id).expect("re-read");
    assert_eq!(unchanged.doc, todo.doc, "the document must be untouched");
    assert_eq!(unchanged.revision, todo.revision, "no revision was burned");
}

/// Two seeded projects, each with one scratchpad, over one façade — the setup the cross-project
/// association guards need. Returns the façade, both project ids, and `b`'s scratchpad id, which is
/// the foreign handle a write against `a` must refuse.
fn two_projects_with_scratchpads() -> (Facade, ProjectId, ProjectId, ScratchpadId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-todo-scope-a"), Some("a"), None)
        .expect("seed the first project")
        .id;
    let b = projects
        .upsert(Path::new("/tmp/soloist-todo-scope-b"), Some("b"), None)
        .expect("seed the second project")
        .id;
    let facade = facade_with(projects);
    facade
        .scratchpad_write_in(b, "other-plan", "the other project's plan".into(), None)
        .expect("write the foreign scratchpad");
    let foreign = facade
        .scratchpad_read_in(b, "other-plan")
        .expect("read it back")
        .id;
    (facade, a, b, foreign)
}

#[test]
fn creating_with_a_scratchpad_from_another_project_is_refused_and_writes_nothing() {
    // The local surface states the association as a durable id, which names a row without naming a
    // project. An id from outside the project must not link, or the board would render another
    // project's document as this todo's origin.
    let (facade, a, _b, foreign) = two_projects_with_scratchpads();

    let refused = facade.todo_create_in(a, doc("ship", TodoStatus::Open), Some(foreign));

    assert!(matches!(refused, Err(CoordinationError::UnknownScratchpad)));
    assert!(
        facade
            .orchestration_snapshot(a)
            .expect("snapshot")
            .todos
            .is_empty(),
        "a refused create must leave no todo behind"
    );
}

#[test]
fn updating_to_a_scratchpad_from_another_project_is_refused_and_writes_nothing() {
    let (facade, a, _b, foreign) = two_projects_with_scratchpads();
    let todo = facade
        .todo_create_in(a, doc("ship", TodoStatus::Open), None)
        .expect("create unlinked");

    let refused = facade.todo_update_in(
        a,
        todo.id,
        doc("renamed", TodoStatus::Open),
        ScratchpadLink::Linked(foreign),
        todo.revision,
    );

    assert!(matches!(refused, Err(CoordinationError::UnknownScratchpad)));
    let unchanged = &facade.orchestration_snapshot(a).expect("snapshot").todos[0];
    assert_eq!(unchanged.doc, todo.doc, "the document must be untouched");
    assert_eq!(unchanged.revision, todo.revision, "no revision was burned");
    assert_eq!(unchanged.scratchpad, None, "and it stays unlinked");
}

#[test]
fn a_scratchpad_in_the_callers_own_project_still_links() {
    // The guard refuses only what is out of scope: the ordinary in-project link is untouched by it.
    let (facade, a, _b, _foreign) = two_projects_with_scratchpads();
    facade
        .scratchpad_write_in(a, "release-plan", "the plan".into(), None)
        .expect("write the local scratchpad");
    let own = facade
        .scratchpad_read_in(a, "release-plan")
        .expect("read it back")
        .id;

    let todo = facade
        .todo_create_in(a, doc("ship", TodoStatus::Open), Some(own))
        .expect("create linked");

    assert_eq!(todo.scratchpad.map(|link| link.id), Some(own));
}

#[test]
fn deleting_a_scratchpad_leaves_the_todos_that_named_it_unlinked() {
    let (facade, session) = scoped_facade();
    let project = ProjectId::from_raw(1);
    facade
        .scratchpad_write_in(project, "release-plan", "the plan".into(), None)
        .expect("write the scratchpad");
    let todo = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), Some("release-plan".into()))
        .expect("create linked")
        .view;
    assert!(todo.scratchpad.is_some());

    assert!(facade
        .scoped(session)
        .scratchpad_delete("release-plan")
        .expect("delete the scratchpad"));

    let after = facade.scoped(session).todo_get(todo.id).expect("re-read");
    assert_eq!(
        after.scratchpad, None,
        "a todo must never point at a document that is gone"
    );
    assert_eq!(after.doc, todo.doc, "and the todo itself survives");
}

#[test]
fn todo_transfer_in_clears_the_scratchpad_association() {
    // The association names a scratchpad in the source project, so like blockers and the lock it
    // cannot survive the move.
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-todo-link-a"), Some("a"), None)
        .expect("seed the source project")
        .id;
    let b = projects
        .upsert(Path::new("/tmp/soloist-todo-link-b"), Some("b"), None)
        .expect("seed the target project")
        .id;
    let facade = facade_with(projects);
    facade
        .scratchpad_write_in(a, "release-plan", "the plan".into(), None)
        .expect("write the scratchpad");
    let pad = facade
        .scratchpad_read_in(a, "release-plan")
        .expect("read it back");
    let todo = facade
        .todo_create_in(a, doc("ship", TodoStatus::Open), Some(pad.id))
        .expect("create linked");
    assert!(todo.scratchpad.is_some());

    let moved = facade
        .todo_transfer_in(a, b, todo.id)
        .expect("transfer across projects");

    assert_eq!(moved.scratchpad, None);
    assert_eq!(moved.doc, todo.doc, "the document rides along unchanged");
}

#[test]
fn a_todo_with_no_scratchpad_is_valid_and_untouched_by_every_path() {
    // Having no scratchpad is a permanent, ordinary state — never a validation failure, never
    // something a later write fills in on the caller's behalf.
    let (facade, session) = scoped_facade();
    let created = facade
        .scoped(session)
        .todo_create(doc("ship", TodoStatus::Open), None)
        .expect("create with no association")
        .view;
    assert_eq!(created.scratchpad, None);

    let updated = facade
        .scoped(session)
        .todo_update(
            created.id,
            doc("ship it", TodoStatus::InProgress),
            ScratchpadLink::Unchanged,
            created.revision,
        )
        .expect("update");
    assert_eq!(updated.scratchpad, None);

    facade
        .scoped(session)
        .todo_add_tag(created.id, "release")
        .expect("tag");
    facade
        .scoped(session)
        .todo_comment_create(created.id, "note")
        .expect("comment");
    let blocker = facade
        .scoped(session)
        .todo_create(doc("dep", TodoStatus::Open), None)
        .expect("create a blocker")
        .view;
    facade
        .scoped(session)
        .todo_set_blockers(created.id, vec![blocker.id])
        .expect("set blockers");
    facade
        .scoped(session)
        .todo_set_blockers(created.id, Vec::new())
        .expect("clear blockers");
    let done = facade
        .scoped(session)
        .todo_complete(created.id)
        .expect("complete");

    assert_eq!(done.scratchpad, None, "no path invented an association");
    assert_eq!(done.doc.status, TodoStatus::Done);
    assert_eq!(done.tags, vec!["release".to_owned()]);
    assert_eq!(done.comments.len(), 1);
}
