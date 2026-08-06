//! Behavioural tests for the façade's version-control read — the door every adapter comes
//! through. They assemble a real [`Facade`] over fakes, so what is asserted is what a Tauri
//! command, an HTTP route, or a tool would get back.

use std::sync::Arc;

use tempfile::TempDir;

use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::ids::ProjectId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{git_status, FakeGitRepository, FakeProjectRepo, FakeSpawner, FakeTrustRepo};

use super::GitStatusError;

/// A façade over `repository` with one project open, returning it and the project's id.
fn facade_with_project(repository: Option<FakeGitRepository>) -> (Facade, ProjectId, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let mut ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(TokioClock),
        Arc::new(FakeTrustRepo::new()),
        projects,
    );
    if let Some(repository) = repository {
        ports = ports.git_repository(Arc::new(repository));
    }
    (Facade::new(ports.build()), project, dir)
}

#[test]
fn an_open_project_reports_the_status_its_repository_holds() {
    let (facade, project, _dir) =
        facade_with_project(Some(FakeGitRepository::reporting(git_status("main"))));

    let status = facade
        .git_status(project)
        .expect("read")
        .expect("a repository");

    assert_eq!(status.branch.name.as_deref(), Some("main"));
}

#[test]
fn a_project_that_is_not_open_is_named_as_such_rather_than_read() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let (facade, _project, _dir) = facade_with_project(Some(repository.clone()));

    assert!(matches!(
        facade.git_status(ProjectId::from_raw(4_242)),
        Err(GitStatusError::UnknownProject),
    ));
    assert_eq!(repository.reads(), 0, "an unknown project is never read");
}

#[test]
fn without_the_git_adapter_a_project_simply_has_no_version_control() {
    let (facade, project, _dir) = facade_with_project(None);

    assert_eq!(
        facade.git_status(project).expect("no error"),
        None,
        "the default port degrades silently, as every optional driven port does",
    );
}
