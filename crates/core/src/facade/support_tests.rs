use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{FakeFeedbackRepo, FakeProjectRepo, FakeSpawner, FakeTrustRepo};

/// A façade over in-memory fakes, with the given project repo and the feedback store wired.
fn facade_over(projects: Arc<FakeProjectRepo>) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .feedback_repo(Arc::new(FakeFeedbackRepo::new()))
        .build(),
    )
}

#[test]
fn submitted_feedback_is_stored_locally_and_listed_back() {
    let facade = facade_over(Arc::new(FakeProjectRepo::new()));

    let entry = facade
        .submit_feedback("  the log pane jumps  ")
        .expect("submit");

    assert_eq!(entry.message, "the log pane jumps");
    assert_eq!(facade.feedback_list().expect("list"), vec![entry]);
}

#[test]
fn blank_feedback_is_refused() {
    let facade = facade_over(Arc::new(FakeProjectRepo::new()));

    assert!(matches!(
        facade.submit_feedback("   "),
        Err(crate::support::FeedbackError::Empty)
    ));
    assert!(facade.feedback_list().expect("list").is_empty());
}

#[test]
fn writing_the_guide_with_no_project_in_scope_is_refused() {
    let facade = facade_over(Arc::new(FakeProjectRepo::new()));
    let session = facade.open_session(None);

    assert!(matches!(
        facade
            .scoped(session)
            .setup_agent_integration(IntegrationFile::ClaudeMd),
        Err(SetupIntegrationError::Scope(
            CoordinationError::NoProjectScope
        ))
    ));
}

#[test]
fn the_guide_lands_in_the_scoped_projects_root_and_reruns_replace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(dir.path(), Some("p"), None)
        .expect("seed one project");
    let facade = facade_over(projects);
    // The sole loaded project gives the unbound session its default scope.
    let session = facade.open_session(None);

    let first = facade
        .scoped(session)
        .setup_agent_integration(IntegrationFile::ClaudeMd)
        .expect("first write");
    let second = facade
        .scoped(session)
        .setup_agent_integration(IntegrationFile::ClaudeMd)
        .expect("second write");

    assert!(first.created);
    assert!(!second.created);
    assert_eq!(first.path, dir.path().join("CLAUDE.md"));
    let contents = std::fs::read_to_string(&first.path).expect("read the written file");
    assert_eq!(
        contents
            .matches("<!-- soloist:integration-guide:begin -->")
            .count(),
        1,
        "re-running must replace the managed section, not duplicate it"
    );
    // The managed section carries the full guide — its identity topic teaches automatic binding.
    assert!(contents.contains("Identity & binding"));
    assert!(contents.contains("whoami"));
}
