use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::ports::{CorePorts, ProjectRepo, TokioClock};
use crate::testing::{FakeProjectRepo, FakeScratchpadRepo, FakeSpawner, FakeTrustRepo};

/// A well-formed disciplined document.
fn doc() -> ScratchpadDoc {
    ScratchpadDoc {
        objective: "Ship v1".into(),
        context: "RC cut".into(),
        plan: vec!["Cut RC".into()],
        acceptance_criteria: vec!["soak green".into()],
        risks: vec!["none identified".into()],
        status: "in progress".into(),
        notes: None,
    }
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
        facade.scratchpad_write(session, "plan", doc(), None),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_session_writes_reads_and_lists_without_binding_a_process() {
    let (facade, session) = scoped_facade();

    let created = facade
        .scratchpad_write(session, "release-plan", doc(), None)
        .expect("create succeeds with only project scope");
    assert_eq!(created.revision, 1);

    let read = facade
        .scratchpad_read(session, "release-plan")
        .expect("read succeeds");
    assert_eq!(read, created);

    let listed = facade.scratchpad_list(session).expect("list succeeds");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "release-plan");
    assert_eq!(listed[0].objective, "Ship v1");
}

#[test]
fn a_stale_write_surfaces_a_revision_conflict() {
    let (facade, session) = scoped_facade();
    facade
        .scratchpad_write(session, "plan", doc(), None)
        .expect("create");
    facade
        .scratchpad_write(session, "plan", doc(), Some(1))
        .expect("first update");

    assert!(matches!(
        facade.scratchpad_write(session, "plan", doc(), Some(1)),
        Err(CoordinationError::RevisionConflict {
            expected: Some(1),
            actual: Some(2)
        })
    ));
}

#[test]
fn a_malformed_write_surfaces_an_invalid_scratchpad() {
    let (facade, session) = scoped_facade();
    let mut bad = doc();
    bad.acceptance_criteria = Vec::new();

    assert!(matches!(
        facade.scratchpad_write(session, "plan", bad, None),
        Err(CoordinationError::InvalidScratchpad(_))
    ));
}

#[test]
fn reading_a_missing_scratchpad_is_unknown() {
    let (facade, session) = scoped_facade();
    assert!(matches!(
        facade.scratchpad_read(session, "absent"),
        Err(CoordinationError::UnknownScratchpad)
    ));
}

#[test]
fn renaming_onto_a_taken_name_is_refused() {
    let (facade, session) = scoped_facade();
    facade
        .scratchpad_write(session, "a", doc(), None)
        .expect("create a");
    facade
        .scratchpad_write(session, "b", doc(), None)
        .expect("create b");

    assert!(matches!(
        facade.scratchpad_rename(session, "a", "b"),
        Err(CoordinationError::ScratchpadNameTaken)
    ));
    assert!(matches!(
        facade.scratchpad_rename(session, "missing", "x"),
        Err(CoordinationError::UnknownScratchpad)
    ));

    let renamed = facade
        .scratchpad_rename(session, "a", "c")
        .expect("rename to a free name succeeds");
    assert_eq!(renamed.name, "c");
}

#[test]
fn tags_and_archive_round_trip_through_the_facade() {
    let (facade, session) = scoped_facade();
    facade
        .scratchpad_write(session, "a", doc(), None)
        .expect("create");

    let tagged = facade
        .scratchpad_add_tags(session, "a", &["release".into()])
        .expect("add tags");
    assert_eq!(tagged.tags, vec!["release".to_string()]);
    assert_eq!(
        facade.scratchpad_tags_list(session).expect("tags list"),
        vec!["release".to_string()]
    );

    let untagged = facade
        .scratchpad_remove_tags(session, "a", &["release".into()])
        .expect("remove tags");
    assert!(untagged.tags.is_empty());

    let archived = facade
        .scratchpad_archive(session, "a", true)
        .expect("archive");
    assert!(archived.archived);

    assert!(facade.scratchpad_delete(session, "a").expect("delete"));
    assert!(matches!(
        facade.scratchpad_add_tags(session, "a", &["x".into()]),
        Err(CoordinationError::UnknownScratchpad)
    ));
}
