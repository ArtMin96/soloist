use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::events::DomainEvent;
use crate::ids::SessionId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    authentic_session, drain, terminal_registration, FakeDiagramRepo, FakeProjectRepo, FakeSpawner,
    FakeTrustRepo, TEST_PEER_PGID,
};
use crate::PeerCredentials;

/// A representative Mermaid source; its first line is the summary gist.
fn source() -> String {
    "sequenceDiagram\n  A->>B: ping\n  B-->>A: pong".to_owned()
}

/// A façade over in-memory fakes with one project loaded and the diagram store wired. The single
/// loaded project gives an unbound session the single-project default scope — so diagrams, which are
/// project-scoped shared content and need no bound owner, work without binding a process.
fn scoped_facade() -> (Facade, SessionId) {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/tmp/soloist-diagram-test"), Some("p"), None)
        .expect("seed one project");
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .diagram_repo(Arc::new(FakeDiagramRepo::new()))
        .build(),
    );
    let session = facade.open_session(PeerCredentials::unauthenticated());
    (facade, session)
}

/// A façade with two projects loaded and the diagram store wired, returning the façade and both
/// project ids — the setup the scope-isolation test shares.
fn two_projects() -> (Facade, ProjectId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let a = projects
        .upsert(Path::new("/tmp/soloist-dg-a"), Some("a"), None)
        .expect("A")
        .id;
    let b = projects
        .upsert(Path::new("/tmp/soloist-dg-b"), Some("b"), None)
        .expect("B")
        .id;
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .diagram_repo(Arc::new(FakeDiagramRepo::new()))
        .build(),
    );
    (facade, a, b)
}

#[test]
fn writing_with_no_project_in_scope_is_refused() {
    // No projects loaded and an unbound session → no effective project to scope the diagram to.
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .diagram_repo(Arc::new(FakeDiagramRepo::new()))
        .build(),
    );
    let session = facade.open_session(PeerCredentials::unauthenticated());

    assert!(matches!(
        facade.scoped(session).diagram_write("flow", source(), None),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_session_writes_reads_and_lists_without_binding_a_process() {
    let (facade, session) = scoped_facade();

    let created = facade
        .scoped(session)
        .diagram_write("flow", source(), None)
        .expect("create succeeds with only project scope");
    assert_eq!(created.revision, 1);
    assert_eq!(created.source, source());

    let read = facade
        .scoped(session)
        .diagram_read("flow")
        .expect("read succeeds");
    assert_eq!(read, created);

    let listed = facade
        .scoped(session)
        .diagram_list()
        .expect("list succeeds");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "flow");
    assert_eq!(listed[0].gist, "sequenceDiagram");
}

#[test]
fn a_stale_write_surfaces_a_revision_conflict() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .diagram_write("flow", source(), None)
        .expect("create");
    facade
        .scoped(session)
        .diagram_write("flow", source(), Some(1))
        .expect("first update");

    assert!(matches!(
        facade
            .scoped(session)
            .diagram_write("flow", source(), Some(1)),
        Err(CoordinationError::DiagramRevisionConflict {
            expected: Some(1),
            actual: Some(2)
        })
    ));
}

#[test]
fn a_malformed_write_surfaces_an_invalid_diagram() {
    let (facade, session) = scoped_facade();
    // A blank name handle is malformed — the source itself is unconstrained.
    assert!(matches!(
        facade.scoped(session).diagram_write("   ", source(), None),
        Err(CoordinationError::InvalidDiagram(_))
    ));
}

#[test]
fn reading_a_missing_diagram_is_unknown() {
    let (facade, session) = scoped_facade();
    assert!(matches!(
        facade.scoped(session).diagram_read("absent"),
        Err(CoordinationError::UnknownDiagram)
    ));
}

#[test]
fn renaming_onto_a_taken_name_is_refused() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .diagram_write("a", source(), None)
        .expect("create a");
    facade
        .scoped(session)
        .diagram_write("b", source(), None)
        .expect("create b");

    assert!(matches!(
        facade.scoped(session).diagram_rename("a", "b"),
        Err(CoordinationError::DiagramNameTaken)
    ));
    assert!(matches!(
        facade.scoped(session).diagram_rename("missing", "x"),
        Err(CoordinationError::UnknownDiagram)
    ));

    let renamed = facade
        .scoped(session)
        .diagram_rename("a", "c")
        .expect("rename to a free name succeeds");
    assert_eq!(renamed.name, "c");
}

#[test]
fn tags_and_archive_round_trip_through_the_facade() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .diagram_write("a", source(), None)
        .expect("create");

    let tagged = facade
        .scoped(session)
        .diagram_add_tags("a", &["arch".into()])
        .expect("add tags");
    assert_eq!(tagged.tags, vec!["arch".to_string()]);
    assert_eq!(
        facade
            .scoped(session)
            .diagram_tags_list()
            .expect("tags list"),
        vec!["arch".to_string()]
    );

    let untagged = facade
        .scoped(session)
        .diagram_remove_tags("a", &["arch".into()])
        .expect("remove tags");
    assert!(untagged.tags.is_empty());

    let archived = facade
        .scoped(session)
        .diagram_archive("a", true)
        .expect("archive");
    assert!(archived.archived);

    assert!(facade.scoped(session).diagram_delete("a").expect("delete"));
    assert!(matches!(
        facade.scoped(session).diagram_add_tags("a", &["x".into()]),
        Err(CoordinationError::UnknownDiagram)
    ));
}

#[test]
fn a_session_scoped_to_one_project_cannot_reach_another_projects_diagrams() {
    let (facade, a, b) = two_projects();
    // Seed a diagram in each project through the trusted local path.
    facade
        .diagram_write_in(a, "in-a", source(), None)
        .expect("create in A");
    facade
        .diagram_write_in(b, "in-b", source(), None)
        .expect("create in B");

    // A session authenticated to A (a process it runs in) is scoped to A only.
    let owner = facade
        .supervisor()
        .register(terminal_registration(a, "w", "sleep 1"));
    let session = authentic_session(&facade, owner, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(owner)
        .expect("bind the session to its process in A");

    // It reads and lists only A's diagram; B's is unreachable by name.
    assert!(facade.scoped(session).diagram_read("in-a").is_ok());
    assert!(matches!(
        facade.scoped(session).diagram_read("in-b"),
        Err(CoordinationError::UnknownDiagram)
    ));
    let listed = facade.scoped(session).diagram_list().expect("list");
    assert_eq!(
        listed.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(),
        vec!["in-a"],
        "the scoped list shows only the session's own project"
    );

    // A write from the session lands in A, never B — B's board is untouched.
    facade
        .scoped(session)
        .diagram_write("new", source(), None)
        .expect("write in A");
    assert!(facade.diagram_read_in(a, "new").is_ok(), "written to A");
    assert!(
        matches!(
            facade.diagram_read_in(b, "new"),
            Err(CoordinationError::UnknownDiagram)
        ),
        "the write never reached B"
    );
}

#[test]
fn a_write_announces_a_diagram_changed_event() {
    let (facade, session) = scoped_facade();
    let project = facade
        .effective_project(session)
        .expect("the single loaded project is the effective scope");
    let mut rx = facade.subscribe();

    facade
        .scoped(session)
        .diagram_write("flow", source(), None)
        .expect("create");

    let events = drain(&mut rx);
    assert!(
        events.iter().any(|event| matches!(
            event,
            DomainEvent::DiagramChanged { project: p, name } if *p == project && name == "flow"
        )),
        "the write emits DiagramChanged for its name: {events:?}"
    );

    // A delete of the same diagram also announces the change.
    let mut rx = facade.subscribe();
    assert!(facade
        .scoped(session)
        .diagram_delete("flow")
        .expect("delete"));
    let events = drain(&mut rx);
    assert!(
        events.iter().any(
            |event| matches!(event, DomainEvent::DiagramChanged { name, .. } if name == "flow")
        ),
        "the delete emits DiagramChanged: {events:?}"
    );
}
