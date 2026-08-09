//! Behavioural tests for the façade's version-control reads and the file actions behind the
//! project's trust gate — the door every adapter comes through. They assemble a real [`Facade`]
//! over fakes, so what is asserted is what a Tauri command, an HTTP route, or a tool would get back.

use std::sync::Arc;

use tempfile::TempDir;

use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::git::{DiffExtent, GitWriteError};
use crate::ids::ProjectId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    file_change, git_status, raw_diff, FakeFileOpener, FakeGitRepository, FakeProjectRepo,
    FakeSpawner, FakeTrustRepo,
};
use crate::vcs::{ChangeKind, DiffTarget, FileContent};

use super::GitReadError;

const PATH: &str = "src/main.rs";

const HEADER: &str =
    "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";

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
        Err(GitReadError::UnknownProject),
    ));
    assert_eq!(repository.reads(), 0, "an unknown project is never read");
}

#[test]
fn an_open_project_reports_the_diff_of_one_of_its_paths() {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, None, Some(ChangeKind::Modified)));
    let repository = FakeGitRepository::reporting(status)
        .diffing(raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n+new\n"]));
    let (facade, project, _dir) = facade_with_project(Some(repository));

    let diff = facade
        .git_diff(project, PATH, DiffTarget::Unstaged, DiffExtent::Capped)
        .expect("read")
        .expect("a repository");

    assert_eq!(diff.path, PATH);
    assert!(diff.patch.contains("+new"));
}

#[test]
fn an_open_project_reports_the_contents_of_one_of_its_files() {
    let repository = FakeGitRepository::reporting(git_status("main")).holding(FileContent {
        text: Some("fn main() {}\n".to_string()),
        truncated: false,
    });
    let (facade, project, _dir) = facade_with_project(Some(repository));

    let content = facade
        .git_file(project, PATH)
        .expect("read")
        .expect("a file");

    assert_eq!(content.text.as_deref(), Some("fn main() {}\n"));
    assert!(!content.truncated);
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
/// A façade over a trusted project, with `repository` behind the working tree and `opener` behind
/// the desktop — the two ports the trust-gated file actions reach.
fn facade_for_a_trusted_project(
    repository: FakeGitRepository,
    opener: FakeFileOpener,
) -> (Facade, ProjectId, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(TokioClock),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
        projects,
    )
    .git_repository(Arc::new(repository))
    .file_opener(Arc::new(opener));
    (Facade::new(ports.build()), project, dir)
}

#[test]
fn opening_a_file_reaches_the_desktop_with_the_path_the_reader_named() {
    let opener = FakeFileOpener::new();
    let (facade, project, _dir) = facade_for_a_trusted_project(
        FakeGitRepository::reporting(git_status("main")),
        opener.clone(),
    );

    facade.git_open_file(project, PATH).expect("open");

    assert_eq!(opener.opened(), vec![PATH.to_string()]);
}

#[test]
fn opening_a_file_in_a_project_that_is_not_open_reaches_no_desktop() {
    let opener = FakeFileOpener::new();
    let (facade, _project, _dir) = facade_for_a_trusted_project(
        FakeGitRepository::reporting(git_status("main")),
        opener.clone(),
    );

    let refusal = facade
        .git_open_file(ProjectId::from_raw(4_242), PATH)
        .expect_err("no such project");

    assert!(
        matches!(refusal, GitWriteError::UnknownProject),
        "{refusal:?}"
    );
    assert_eq!(opener.opened(), Vec::<String>::new());
}

#[test]
fn a_message_box_starts_at_what_the_repository_configured_it_to_start_at() {
    const TEMPLATE: &str = "Refs:\n";
    let (facade, project, _dir) = facade_for_a_trusted_project(
        FakeGitRepository::reporting(git_status("main")).templating(TEMPLATE),
        FakeFileOpener::new(),
    );

    let offered = facade
        .git_commit_template(project)
        .expect("a template read");

    assert_eq!(offered, Some(TEMPLATE.to_string()));
}
