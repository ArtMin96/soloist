use crate::facade::Facade;
use crate::ids::SessionId;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{TodoDoc, TodoStatus, TodoView};
use crate::events::DomainEvent;
use crate::ids::TodoId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, drain, terminal_registration, FakeProjectRepo, FakeScratchpadRepo,
    FakeSpawner, FakeTodoRepo, FakeTrustRepo, TEST_PEER_PGID,
};

/// A representative Markdown body; its first non-heading line is the summary gist.
fn body() -> String {
    "## Objective\nShip v1\n\n## Status\nin progress".to_owned()
}

/// A façade over in-memory fakes with one project loaded and the scratchpad store wired. The single
/// loaded project gives an unbound session the single-project default scope — so scratchpads, which
/// are project-scoped shared content and need no bound owner, work without binding a process.
fn scoped_facade() -> (Facade, SessionId) {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/tmp/soloist-scratchpad-test"), Some("p"), None)
        .expect("seed one project");
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .scratchpad_repo(Arc::new(FakeScratchpadRepo::new()))
        .build(),
    );
    let session = facade.open_session(None);
    (facade, session)
}

#[test]
fn writing_with_no_project_in_scope_is_refused() {
    // No projects loaded and an unbound session → no effective project to scope the scratchpad to.
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .scratchpad_repo(Arc::new(FakeScratchpadRepo::new()))
        .build(),
    );
    let session = facade.open_session(None);

    assert!(matches!(
        facade
            .scoped(session)
            .scratchpad_write("plan", body(), None),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_session_writes_reads_and_lists_without_binding_a_process() {
    let (facade, session) = scoped_facade();

    let created = facade
        .scoped(session)
        .scratchpad_write("release-plan", body(), None)
        .expect("create succeeds with only project scope")
        .view;
    assert_eq!(created.revision, 1);

    let read = facade
        .scoped(session)
        .scratchpad_read("release-plan")
        .expect("read succeeds");
    assert_eq!(read, created);

    let listed = facade
        .scoped(session)
        .scratchpad_list()
        .expect("list succeeds");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "release-plan");
    assert_eq!(listed[0].gist, "Ship v1");
}

#[test]
fn a_stale_write_surfaces_a_revision_conflict() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .scratchpad_write("plan", body(), None)
        .expect("create");
    facade
        .scoped(session)
        .scratchpad_write("plan", body(), Some(1))
        .expect("first update");

    assert!(matches!(
        facade
            .scoped(session)
            .scratchpad_write("plan", body(), Some(1)),
        Err(CoordinationError::RevisionConflict {
            expected: Some(1),
            actual: Some(2)
        })
    ));
}

#[test]
fn a_malformed_write_surfaces_an_invalid_scratchpad() {
    let (facade, session) = scoped_facade();
    // A blank name handle is malformed — the body itself is unconstrained.
    assert!(matches!(
        facade.scoped(session).scratchpad_write("   ", body(), None),
        Err(CoordinationError::InvalidScratchpad(_))
    ));
}

#[test]
fn reading_a_missing_scratchpad_is_unknown() {
    let (facade, session) = scoped_facade();
    assert!(matches!(
        facade.scoped(session).scratchpad_read("absent"),
        Err(CoordinationError::UnknownScratchpad)
    ));
}

#[test]
fn renaming_onto_a_taken_name_is_refused() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .scratchpad_write("a", body(), None)
        .expect("create a");
    facade
        .scoped(session)
        .scratchpad_write("b", body(), None)
        .expect("create b");

    assert!(matches!(
        facade.scoped(session).scratchpad_rename("a", "b"),
        Err(CoordinationError::ScratchpadNameTaken)
    ));
    assert!(matches!(
        facade.scoped(session).scratchpad_rename("missing", "x"),
        Err(CoordinationError::UnknownScratchpad)
    ));

    let renamed = facade
        .scoped(session)
        .scratchpad_rename("a", "c")
        .expect("rename to a free name succeeds");
    assert_eq!(renamed.name, "c");
}

#[test]
fn scratchpad_rename_in_renames_the_document_keeping_its_identity_and_revision() {
    let (facade, a, _) = two_projects();
    let created = facade
        .scratchpad_write_in(a, "release-plan", body(), None)
        .expect("create");

    let renamed = facade
        .scratchpad_rename_in(a, "release-plan", "Release plan")
        .expect("rename to a free name succeeds");
    assert_eq!(renamed.name, "Release plan");
    assert_eq!(renamed.id, created.id, "the durable id is unchanged");
    assert_eq!(
        renamed.revision, created.revision,
        "renaming is not an edit"
    );
    assert_eq!(renamed.body, created.body, "the body is untouched");

    // It reads back under the new handle only — the old one is gone.
    assert_eq!(
        facade
            .scratchpad_read_in(a, "Release plan")
            .expect("reads under the new name"),
        renamed
    );
    assert!(matches!(
        facade.scratchpad_read_in(a, "release-plan"),
        Err(CoordinationError::UnknownScratchpad)
    ));
}

#[test]
fn scratchpad_rename_in_refuses_a_taken_name_and_an_unknown_scratchpad() {
    let (facade, a, _) = two_projects();
    facade
        .scratchpad_write_in(a, "plan", body(), None)
        .expect("create plan");
    facade
        .scratchpad_write_in(a, "research", body(), None)
        .expect("create research");

    assert!(matches!(
        facade.scratchpad_rename_in(a, "plan", "research"),
        Err(CoordinationError::ScratchpadNameTaken)
    ));
    assert!(matches!(
        facade.scratchpad_rename_in(a, "missing", "free"),
        Err(CoordinationError::UnknownScratchpad)
    ));

    // Neither refusal moved anything: both documents still read under their own names.
    assert!(facade.scratchpad_read_in(a, "plan").is_ok());
    assert!(facade.scratchpad_read_in(a, "research").is_ok());
    assert!(matches!(
        facade.scratchpad_read_in(a, "free"),
        Err(CoordinationError::UnknownScratchpad)
    ));
}

#[test]
fn tags_and_archive_round_trip_through_the_facade() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .scratchpad_write("a", body(), None)
        .expect("create");

    let tagged = facade
        .scoped(session)
        .scratchpad_add_tags("a", &["release".into()])
        .expect("add tags");
    assert_eq!(tagged.tags, vec!["release".to_string()]);
    assert_eq!(
        facade
            .scoped(session)
            .scratchpad_tags_list()
            .expect("tags list"),
        vec!["release".to_string()]
    );

    let untagged = facade
        .scoped(session)
        .scratchpad_remove_tags("a", &["release".into()])
        .expect("remove tags");
    assert!(untagged.tags.is_empty());

    let archived = facade
        .scoped(session)
        .scratchpad_archive("a", true)
        .expect("archive");
    assert!(archived.archived);

    assert!(facade
        .scoped(session)
        .scratchpad_delete("a")
        .expect("delete"));
    assert!(matches!(
        facade
            .scoped(session)
            .scratchpad_add_tags("a", &["x".into()]),
        Err(CoordinationError::UnknownScratchpad)
    ));
}

/// A façade with two projects loaded and the scratchpad *and* todo stores wired over one shared row
/// set, returning the façade and both project ids — the setup the transfer tests share. Both stores
/// are wired because a transfer moves the todos derived from the scratchpad along with it.
fn two_projects() -> (Facade, ProjectId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-sp-a"), Some("a"), None)
        .expect("A")
        .id;
    let b = projects
        .upsert(Path::new("/tmp/soloist-sp-b"), Some("b"), None)
        .expect("B")
        .id;
    let scratchpads = Arc::new(FakeScratchpadRepo::new());
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .todo_repo(Arc::new(FakeTodoRepo::joined(Arc::clone(&scratchpads))))
        .scratchpad_repo(scratchpads)
        .build(),
    );
    (facade, a, b)
}

/// Creates the scratchpad `name` in `project` and a todo titled `title` derived from it, returning
/// the todo's id — the two-sided setup every cascade assertion starts from.
fn derived_todo(facade: &Facade, project: ProjectId, name: &str, title: &str) -> TodoId {
    let pad = facade
        .scratchpad_write_in(project, name, body(), None)
        .expect("create the scratchpad");
    facade
        .todo_create_in(project, todo(title), Some(pad.id))
        .expect("create a todo derived from it")
        .id
}

/// A minimal valid todo document.
fn todo(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.to_owned(),
        body: "do it".to_owned(),
        status: TodoStatus::Open,
    }
}

/// The todos `project`'s board shows, in creation order.
fn board(facade: &Facade, project: ProjectId) -> Vec<TodoView> {
    facade
        .orchestration_snapshot(project)
        .expect("snapshot")
        .todos
}

#[test]
fn scratchpad_transfer_in_moves_the_document_keeping_its_identity_and_revision() {
    let (facade, a, b) = two_projects();
    let created = facade
        .scratchpad_write_in(a, "plan", body(), None)
        .expect("create in A");

    let moved = facade
        .scratchpad_transfer_in(a, "plan", b)
        .expect("transfer");
    assert_eq!(moved.id, created.id, "the durable id is stable");
    assert_eq!(moved.name, "plan", "the name handle is kept");
    assert_eq!(
        moved.revision, created.revision,
        "the revision is preserved"
    );
    assert_eq!(moved.body, created.body, "the body is preserved");

    // It now reads from B and is gone from A.
    assert!(facade.scratchpad_read_in(b, "plan").is_ok());
    assert!(matches!(
        facade.scratchpad_read_in(a, "plan"),
        Err(CoordinationError::UnknownScratchpad)
    ));
}

#[test]
fn scratchpad_transfer_in_refuses_a_name_already_used_in_the_target() {
    let (facade, a, b) = two_projects();
    facade
        .scratchpad_write_in(a, "plan", body(), None)
        .expect("create in A");
    facade
        .scratchpad_write_in(b, "plan", body(), None)
        .expect("a scratchpad already named plan in B");

    assert!(matches!(
        facade.scratchpad_transfer_in(a, "plan", b),
        Err(CoordinationError::ScratchpadNameTaken)
    ));
}

#[test]
fn scratchpad_transfer_refuses_a_target_outside_the_callers_authenticated_scope() {
    let (facade, a, b) = two_projects();
    // The session authenticates to A (a process it runs in), never B.
    let owner = facade
        .supervisor()
        .register(terminal_registration(a, "w", "sleep 1"));
    let session = authentic_session(&facade, owner, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(owner)
        .expect("bind the session to its process in A");
    let derived = derived_todo(&facade, a, "plan", "ship");

    assert!(matches!(
        facade.scoped(session).scratchpad_transfer("plan", b),
        Err(CoordinationError::ForeignProject)
    ));

    // The refusal is decided before anything moves, so neither the document nor the work derived
    // from it left A.
    assert!(facade.scratchpad_read_in(a, "plan").is_ok(), "still in A");
    assert_eq!(
        board(&facade, a).iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![derived],
        "the derived todo stayed in A"
    );
    assert!(board(&facade, b).is_empty(), "nothing was written to B");
}

#[test]
fn scratchpad_transfer_in_takes_the_todos_derived_from_it_and_keeps_their_link() {
    let (facade, a, b) = two_projects();
    let derived = derived_todo(&facade, a, "plan", "ship");
    // A todo deriving from a different scratchpad, and one deriving from nothing at all, both of
    // which must be left exactly where they are.
    let other = derived_todo(&facade, a, "notes", "tidy");
    let loose = facade
        .todo_create_in(a, todo("triage"), None)
        .expect("an unlinked todo")
        .id;

    let moved = facade
        .scratchpad_transfer_in(a, "plan", b)
        .expect("transfer");

    let target = board(&facade, b);
    assert_eq!(
        target.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![derived],
        "only the todos derived from the moved scratchpad went with it"
    );
    let link = target[0]
        .scratchpad
        .as_ref()
        .expect("the association survives the move");
    assert_eq!(link.id, moved.id, "still derived from the same document");
    assert_eq!(link.name, "plan", "and still resolves to its handle");

    let source: Vec<TodoId> = board(&facade, a).iter().map(|t| t.id).collect();
    assert_eq!(
        source,
        vec![other, loose],
        "todos deriving from another scratchpad or from none are untouched"
    );
}

#[test]
fn scratchpad_transfer_in_announces_the_moved_todos_on_both_boards() {
    let (facade, a, b) = two_projects();
    let derived = derived_todo(&facade, a, "plan", "ship");
    let mut rx = facade.subscribe();

    facade
        .scratchpad_transfer_in(a, "plan", b)
        .expect("transfer");

    let events = drain(&mut rx);
    for project in [a, b] {
        assert!(
            events.iter().any(|event| matches!(
                event,
                DomainEvent::ScratchpadChanged { project: p, name } if *p == project && name == "plan"
            )),
            "the scratchpad change reaches {project:?}: {events:?}"
        );
        assert!(
            events.iter().any(|event| matches!(
                event,
                DomainEvent::TodoChanged { project: p, id } if *p == project && *id == derived
            )),
            "the moved todo is announced on {project:?}: {events:?}"
        );
    }
}

#[test]
fn scratchpad_transfer_in_refuses_an_unknown_target_project() {
    let (facade, a, _b) = two_projects();
    facade
        .scratchpad_write_in(a, "plan", body(), None)
        .expect("create in A");

    // A target project that is not loaded is refused before any move, so a bad id never orphans
    // the scratchpad — it stays readable in A.
    assert!(matches!(
        facade.scratchpad_transfer_in(a, "plan", ProjectId::from_raw(9999)),
        Err(CoordinationError::UnknownProject)
    ));
    assert!(facade.scratchpad_read_in(a, "plan").is_ok(), "still in A");
}
