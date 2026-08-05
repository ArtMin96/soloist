//! Behavioural tests for the git context's status cache, kept out of the implementation file.
//! They drive a real [`Git`] over the shared [`FakeGitRepository`], so what is asserted is what
//! a caller observes — the status served, how often the port was actually read, and how many
//! reads were ever in flight at once.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::ids::ProjectId;
use crate::testing::{git_status, FakeGitRepository};
use crate::vcs::{ChangeKind, FileChange, GitFileStatus};

use super::{Git, GitStatus};
use crate::git::GitError;

/// The fake ignores it — a status read is addressed by project here, not by path.
const ROOT: &str = "/project";

fn git(repository: &FakeGitRepository) -> Git {
    Git::new(Arc::new(repository.clone()))
}

fn with_change(mut status: GitStatus, path: &str) -> GitStatus {
    status.changes.push(FileChange {
        path: path.to_string(),
        status: GitFileStatus {
            staged: None,
            unstaged: Some(ChangeKind::Modified),
        },
        original_path: None,
    });
    status
}

#[test]
fn a_second_look_is_served_without_reading_the_repository_again() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let git = git(&repository);
    let project = ProjectId::next();

    let first = git.status(project, Path::new(ROOT)).expect("read");
    let second = git.status(project, Path::new(ROOT)).expect("read");

    assert_eq!(first, second);
    assert_eq!(
        first.expect("a repository").branch.name.as_deref(),
        Some("main")
    );
    assert_eq!(
        repository.reads(),
        1,
        "the remembered status served the second look"
    );
}

#[test]
fn a_project_that_is_not_a_repository_reads_as_nothing_to_show_and_stays_that_way() {
    let repository = FakeGitRepository::answering(vec![Err(GitError::NotARepo)]);
    let git = git(&repository);
    let project = ProjectId::next();

    assert_eq!(
        git.status(project, Path::new(ROOT)).expect("no error"),
        None
    );
    assert_eq!(
        git.status(project, Path::new(ROOT)).expect("no error"),
        None
    );
    assert_eq!(
        repository.reads(),
        1,
        "a folder kept out of version control is not re-read on every look",
    );
}

#[test]
fn a_refresh_reports_a_change_only_when_the_working_tree_reads_differently() {
    let clean = git_status("main");
    let repository = FakeGitRepository::answering(vec![
        Ok(clean.clone()),
        Ok(clean.clone()),
        Ok(with_change(clean, "src/main.rs")),
    ]);
    let git = git(&repository);
    let project = ProjectId::next();

    assert!(
        git.refresh(project, Path::new(ROOT)).expect("read"),
        "the first read is new by definition",
    );
    assert!(
        !git.refresh(project, Path::new(ROOT)).expect("read"),
        "the same working tree read again is not a change",
    );
    assert!(
        git.refresh(project, Path::new(ROOT)).expect("read"),
        "a newly modified file is a change",
    );
    assert_eq!(
        git.status(project, Path::new(ROOT))
            .expect("read")
            .expect("a repository")
            .changes
            .len(),
        1,
    );
}

#[test]
fn a_failed_read_surfaces_and_leaves_the_last_known_status_standing() {
    let repository =
        FakeGitRepository::answering(vec![Ok(git_status("main")), Err(GitError::Timeout)]);
    let git = git(&repository);
    let project = ProjectId::next();

    git.refresh(project, Path::new(ROOT)).expect("first read");
    assert!(matches!(
        git.refresh(project, Path::new(ROOT)),
        Err(GitError::Timeout)
    ));

    let after = git.status(project, Path::new(ROOT)).expect("read");
    assert_eq!(
        after.expect("a repository").branch.name.as_deref(),
        Some("main"),
        "a momentary failure must not blank a rail that was showing the truth",
    );
    assert_eq!(
        repository.reads(),
        2,
        "the remembered status answered without a third read",
    );
}

#[test]
fn forgetting_a_project_makes_the_next_look_read_again() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let git = git(&repository);
    let project = ProjectId::next();

    git.status(project, Path::new(ROOT)).expect("read");
    git.forget(project);
    git.status(project, Path::new(ROOT)).expect("read");

    assert_eq!(repository.reads(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reads_against_one_repository_never_overlap() {
    // Each read dwells long enough that eight of them, released together, would overlap were
    // they not gated — so a peak of one is evidence of serialization, not of luck.
    let repository = FakeGitRepository::slow(git_status("main"), Duration::from_millis(20));
    let git = Arc::new(git(&repository));
    let project = ProjectId::next();

    let mut readers = Vec::new();
    for _ in 0..8 {
        let git = git.clone();
        readers.push(tokio::task::spawn_blocking(move || {
            git.refresh(project, Path::new(ROOT))
        }));
    }
    for reader in readers {
        reader.await.expect("reader finished").expect("read");
    }

    assert_eq!(repository.reads(), 8);
    assert_eq!(
        repository.peak_concurrent(),
        1,
        "one repository is read one call at a time",
    );
}
