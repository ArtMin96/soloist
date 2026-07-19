use crate::facade::Facade;
use crate::ids::SessionId;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, terminal_registration, FakeProjectRepo, FakeScratchpadRepo, FakeSpawner,
    FakeTrustRepo, TEST_PEER_PGID,
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

/// A façade with two projects loaded and the scratchpad store wired, returning the façade and both
/// project ids — the setup the transfer tests share.
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
    (facade, a, b)
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
    facade
        .scratchpad_write_in(a, "plan", body(), None)
        .expect("create in A");

    assert!(matches!(
        facade.scoped(session).scratchpad_transfer("plan", b),
        Err(CoordinationError::ForeignProject)
    ));
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
