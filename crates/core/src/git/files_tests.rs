//! Behavioural tests for the project file listing, driving a real [`Git`] over the shared
//! [`FakeGitRepository`] so what is asserted is what a caller observes.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::ids::ProjectId;
use crate::testing::{git_status, project_file, FakeGitRepository};

use super::Git;

/// The fake ignores it — a read is addressed by project here, not by path.
const ROOT: &str = "/project";

#[test]
fn a_listing_carries_every_path_the_repository_reported_and_which_of_them_are_ignored() {
    let repository = FakeGitRepository::reporting(git_status("main")).listing(vec![
        project_file("src/main.rs", false),
        project_file("target/", true),
    ]);
    let git = Git::new(Arc::new(repository));

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
    let git = Git::new(Arc::new(FakeGitRepository::reporting(git_status("main"))));

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
    let git = Git::new(Arc::new(repository.clone()));
    let project = ProjectId::next();

    git.files(project, Path::new(ROOT)).expect("read");
    git.files(project, Path::new(ROOT)).expect("read");

    assert_eq!(
        repository.reads(),
        2,
        "a listing is shown on demand, so it is read on demand rather than cached",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_listing_and_a_status_never_run_against_one_repository_at_once() {
    // Each read dwells long enough that the two, released together, would overlap were they not
    // sharing the project's gate.
    let repository = FakeGitRepository::slow(git_status("main"), Duration::from_millis(20))
        .listing(vec![project_file("src/main.rs", false)]);
    let git = Arc::new(Git::new(Arc::new(repository.clone())));
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
