//! Behavioural tests for reading one path's diff, driving a real [`Git`] over the shared
//! [`FakeGitRepository`] so what is asserted is what a caller observes.

use std::path::Path;
use std::sync::Arc;

use crate::git::NoopGitForge;
use crate::ids::ProjectId;
use crate::testing::{file_change, git_status, raw_diff, untrusting, FakeGitRepository};
use crate::vcs::{ChangeKind, DiffTarget, FileChange, FileDiff};

use super::{DiffExtent, Git, GitError, RawFileDiff, DIFF_LIMIT};
use crate::git::GitStatus;

/// The fake ignores it — a read is addressed by project here, not by path.
const ROOT: &str = "/project";

const PATH: &str = "src/main.rs";

const HEADER: &str =
    "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";

/// What every hunk below starts with; counting it counts the hunks that survived.
const HUNK_HEADER: &str = "@@ -1,1 +1,1 @@\n";

/// A hunk of roughly `bytes` bytes, so a test can state sizes against the cap rather than
/// against a line count that would have to be re-derived every time the cap moved.
fn hunk(bytes: usize) -> String {
    let body = "+x\n".repeat(bytes.saturating_sub(HUNK_HEADER.len()) / 3);
    format!("{HUNK_HEADER}{body}")
}

/// A working tree whose only change is `change`, over a repository answering `diff`.
fn git_over(change: Option<FileChange>, diff: RawFileDiff) -> (Git, FakeGitRepository) {
    let mut status: GitStatus = git_status("main");
    status.changes.extend(change);
    let repository = FakeGitRepository::reporting(status).diffing(diff);
    (
        Git::new(
            Arc::new(repository.clone()),
            Arc::new(NoopGitForge),
            untrusting(),
        ),
        repository,
    )
}

/// The one call under test, so each case states only what it varies.
fn read(git: &Git, path: &str, target: DiffTarget, extent: DiffExtent) -> Option<FileDiff> {
    git.diff(ProjectId::next(), Path::new(ROOT), path, target, extent)
        .expect("read")
}

#[test]
fn a_diff_carries_the_patch_its_repository_produced_header_and_all() {
    let (git, _repository) = git_over(
        Some(file_change(PATH, None, Some(ChangeKind::Modified))),
        raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n+new\n"]),
    );

    let diff = read(&git, PATH, DiffTarget::Unstaged, DiffExtent::Capped).expect("a repository");

    assert_eq!(
        diff.patch,
        format!("{HEADER}@@ -1,1 +1,1 @@\n-old\n+new\n"),
        "the patch a reader is handed is the one version control produced",
    );
    assert!(!diff.truncated);
    assert_eq!(diff.target, DiffTarget::Unstaged);
}

#[test]
fn an_untracked_path_is_read_as_the_whole_of_itself_whatever_was_asked_for() {
    let (git, _repository) = git_over(
        Some(file_change(PATH, None, Some(ChangeKind::Untracked))),
        raw_diff(HEADER, &["@@ -0,0 +1,1 @@\n+new\n"]),
    );

    let diff = read(&git, PATH, DiffTarget::Staged, DiffExtent::Capped).expect("a repository");

    assert_eq!(
        diff.target,
        DiffTarget::Untracked,
        "nothing earlier exists to compare an untracked path against, so the answer says which \
         comparison it really is rather than the one that was asked for",
    );
}

#[test]
fn a_tracked_path_is_read_at_the_comparison_that_was_asked_for() {
    let (git, _repository) = git_over(
        Some(file_change(PATH, Some(ChangeKind::Modified), None)),
        raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n+new\n"]),
    );

    let diff = read(&git, PATH, DiffTarget::Staged, DiffExtent::Capped).expect("a repository");

    assert_eq!(diff.target, DiffTarget::Staged);
}

#[test]
fn a_diff_longer_than_one_read_carries_arrives_as_whole_hunks_and_says_there_are_more() {
    let third = hunk(DIFF_LIMIT / 3);
    let (git, _repository) = git_over(
        Some(file_change(PATH, None, Some(ChangeKind::Modified))),
        raw_diff(HEADER, &[&third, &third, &third]),
    );

    let diff = read(&git, PATH, DiffTarget::Unstaged, DiffExtent::Capped).expect("a repository");

    assert!(diff.truncated, "a reader is told the diff goes on");
    assert!(
        diff.patch.starts_with(HEADER),
        "what is carried is still a patch, header included",
    );
    assert_eq!(
        diff.patch.matches(HUNK_HEADER).count(),
        2,
        "the cut falls between hunks, never inside one",
    );
}

#[test]
fn asking_for_the_whole_diff_carries_all_of_it() {
    let third = hunk(DIFF_LIMIT / 3);
    let (git, _repository) = git_over(
        Some(file_change(PATH, None, Some(ChangeKind::Modified))),
        raw_diff(HEADER, &[&third, &third, &third]),
    );

    let diff = read(&git, PATH, DiffTarget::Unstaged, DiffExtent::Full).expect("a repository");

    assert!(!diff.truncated);
    assert_eq!(diff.patch.matches(HUNK_HEADER).count(), 3);
}

#[test]
fn one_hunk_past_the_cap_is_still_carried_rather_than_leaving_a_header_alone() {
    let enormous = hunk(DIFF_LIMIT * 2);
    let (git, _repository) = git_over(
        Some(file_change(PATH, None, Some(ChangeKind::Modified))),
        raw_diff(HEADER, &[&enormous]),
    );

    let diff = read(&git, PATH, DiffTarget::Unstaged, DiffExtent::Capped).expect("a repository");

    assert_eq!(
        diff.patch.matches(HUNK_HEADER).count(),
        1,
        "a header with no hunk under it says a file changed while showing nothing of how",
    );
    assert!(!diff.truncated, "nothing was left out — there was one hunk");
}

#[test]
fn a_binary_path_carries_no_patch_at_all() {
    let mut binary = raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n"]);
    binary.binary = true;
    let (git, _repository) = git_over(
        Some(file_change(PATH, None, Some(ChangeKind::Modified))),
        binary,
    );

    let diff = read(&git, PATH, DiffTarget::Unstaged, DiffExtent::Capped).expect("a repository");

    assert!(diff.binary);
    assert_eq!(
        diff.patch, "",
        "there is nothing in a binary file a reader could be shown but noise",
    );
}

#[test]
fn a_path_that_climbs_out_of_the_repository_is_never_read() {
    let (git, repository) = git_over(None, raw_diff(HEADER, &[]));

    let diff = read(
        &git,
        "../../etc/passwd",
        DiffTarget::Unstaged,
        DiffExtent::Capped,
    );

    assert_eq!(diff, None);
    assert_eq!(
        repository.reads(),
        0,
        "a path outside the repository reaches no read at all",
    );
}

#[test]
fn a_project_that_is_not_a_repository_has_no_diff_rather_than_failing() {
    let git = Git::new(
        Arc::new(FakeGitRepository::answering(vec![Err(GitError::NotARepo)])),
        Arc::new(NoopGitForge),
        untrusting(),
    );

    assert_eq!(
        read(&git, PATH, DiffTarget::Unstaged, DiffExtent::Capped),
        None,
    );
}
