//! Behavioural tests for the project file listing, driving a real [`Git`] over the shared
//! [`FakeGitRepository`] so what is asserted is what a caller observes.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::git::{NoopFileOpener, NoopGitForge, OpenError};
use crate::ids::ProjectId;
use crate::testing::{
    git_opening, git_status, project_file, untrusting, FakeFileOpener, FakeGitRepository,
};
use crate::vcs::FileContent;

use super::{Git, GitWriteError};

/// The fake ignores it — a read is addressed by project here, not by path.
const ROOT: &str = "/project";

#[test]
fn a_listing_carries_every_path_the_repository_reported_and_which_of_them_are_ignored() {
    let repository = FakeGitRepository::reporting(git_status("main")).listing(vec![
        project_file("src/main.rs", false),
        project_file("target/", true),
    ]);
    let git = Git::new(
        Arc::new(repository),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    );

    let files = git
        .files(ProjectId::next(), Path::new(ROOT))
        .expect("read")
        .expect("a repository");

    assert_eq!(
        files
            .iter()
            .map(|file| (file.path.as_str(), file.ignored))
            .collect::<Vec<_>>(),
        vec![("src/main.rs", false), ("target/", true)],
    );
}

#[test]
fn a_project_that_is_not_a_repository_lists_nothing_rather_than_failing() {
    // The fake lists nothing unless told to, which is how a folder under no version control
    // answers.
    let git = Git::new(
        Arc::new(FakeGitRepository::reporting(git_status("main"))),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    );

    assert_eq!(
        git.files(ProjectId::next(), Path::new(ROOT))
            .expect("no error"),
        None
    );
}

#[test]
fn a_listing_is_read_afresh_rather_than_remembered() {
    let repository = FakeGitRepository::reporting(git_status("main"))
        .listing(vec![project_file("src/main.rs", false)]);
    let git = Git::new(
        Arc::new(repository.clone()),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    );
    let project = ProjectId::next();

    git.files(project, Path::new(ROOT)).expect("read");
    git.files(project, Path::new(ROOT)).expect("read");

    assert_eq!(
        repository.reads(),
        2,
        "a listing is shown on demand, so it is read on demand rather than cached",
    );
}

#[test]
fn a_listed_file_is_read_as_the_working_tree_holds_it() {
    let content = FileContent {
        text: Some("fn main() {}\n".to_string()),
        truncated: false,
    };
    let repository = FakeGitRepository::reporting(git_status("main")).holding(content.clone());
    let git = Git::new(
        Arc::new(repository),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    );

    assert_eq!(
        git.file(ProjectId::next(), Path::new(ROOT), "src/main.rs")
            .expect("read"),
        Some(content),
    );
}

#[test]
fn a_path_that_climbs_out_of_the_repository_is_never_read() {
    let repository = FakeGitRepository::reporting(git_status("main")).holding(FileContent {
        text: Some("root:x:0:0".to_string()),
        truncated: false,
    });
    let git = Git::new(
        Arc::new(repository.clone()),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    );

    let content = git
        .file(ProjectId::next(), Path::new(ROOT), "../../etc/passwd")
        .expect("no error");

    assert_eq!(content, None);
    assert_eq!(
        repository.reads(),
        0,
        "a path outside the repository reaches no read at all",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_listing_and_a_status_never_run_against_one_repository_at_once() {
    // Each read dwells long enough that the two, released together, would overlap were they not
    // sharing the project's gate.
    let repository = FakeGitRepository::slow(git_status("main"), Duration::from_millis(20))
        .listing(vec![project_file("src/main.rs", false)]);
    let git = Arc::new(Git::new(
        Arc::new(repository.clone()),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    ));
    let project = ProjectId::next();

    let listing = {
        let git = git.clone();
        tokio::task::spawn_blocking(move || git.files(project, Path::new(ROOT)))
    };
    let status = {
        let git = git.clone();
        tokio::task::spawn_blocking(move || git.refresh(project, Path::new(ROOT)))
    };
    listing.await.expect("reader finished").expect("read");
    status.await.expect("reader finished").expect("read");

    assert_eq!(repository.reads(), 2);
    assert_eq!(
        repository.peak_concurrent(),
        1,
        "one repository is read one call at a time, whichever call it is",
    );
}

#[test]
fn a_file_a_reader_asked_for_is_handed_to_the_desktop_by_the_name_it_has_in_the_project() {
    let opener = FakeFileOpener::new();
    let project = ProjectId::next();
    let git = git_opening(opener.clone(), project, true);

    git.open_file(project, Path::new(ROOT), "src/main.rs")
        .expect("open");

    assert_eq!(opener.opened(), vec!["src/main.rs".to_string()]);
}

#[test]
fn a_project_that_has_not_been_trusted_opens_nothing_anywhere() {
    // Opening a file starts whichever program the desktop picked for it, on contents the
    // repository supplied. A project the user has not authorised Soloist to act within does not
    // get to choose a program for them.
    let opener = FakeFileOpener::new();
    let project = ProjectId::next();
    let git = git_opening(opener.clone(), project, false);

    let refusal = git
        .open_file(project, Path::new(ROOT), "src/main.rs")
        .unwrap_err();

    assert!(matches!(refusal, GitWriteError::Untrusted), "{refusal:?}");
    assert_eq!(opener.opened(), Vec::<String>::new());
}

#[test]
fn a_path_that_says_it_leaves_the_repository_never_reaches_the_desktop() {
    let opener = FakeFileOpener::new();
    let project = ProjectId::next();
    let git = git_opening(opener.clone(), project, true);

    let refusal = git
        .open_file(project, Path::new(ROOT), "../../etc/passwd")
        .unwrap_err();

    assert!(
        matches!(refusal, GitWriteError::OutsideRepository),
        "{refusal:?}"
    );
    assert_eq!(opener.opened(), Vec::<String>::new());
}

#[test]
fn a_path_that_only_leads_out_of_the_repository_is_refused_where_that_can_be_seen() {
    // Every component of the name is ordinary, so nothing the core can check says it leaves —
    // which is why following it is the port's job, and why what the port says is refused rather
    // than reported as a file that would not open.
    let opener = FakeFileOpener::refusing(OpenError::Outside);
    let project = ProjectId::next();
    let git = git_opening(opener, project, true);

    let refusal = git.open_file(project, Path::new(ROOT), "key").unwrap_err();

    assert!(
        matches!(refusal, GitWriteError::OutsideRepository),
        "{refusal:?}"
    );
}
