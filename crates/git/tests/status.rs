//! The adapter read against real repositories, each built in a temporary directory by the same
//! `git` a user would run. Nothing here is a recording: the fixtures are made with real commits,
//! renames, merges, and a real remote, so what is asserted is what the installed tool actually
//! reports.

use std::path::{Path, PathBuf};

use soloist_core::{ChangeKind, FileChange, GitError, GitRepository, GitStatus, SyncState};
use soloist_git::CliGitRepository;
use tempfile::TempDir;

mod fixture;
use fixture::{git, repository_with, try_git, write, BRANCH};

const OVERSIZED_UNTRACKED_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// A clone of a bare repository with one commit pushed to it, returned as (the whole fixture,
/// the working copy) so the remote outlives the test.
fn clone_of_a_remote() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir");
    let remote = dir.path().join("remote");
    let work = dir.path().join("work");
    std::fs::create_dir(&remote).expect("create remote");
    git(&remote, &["init", "--bare", "-b", BRANCH]);
    git(dir.path(), &["clone", "remote", "work"]);
    write(&work, "a.txt", "one\n");
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-m", "start"]);
    git(&work, &["push", "-u", "origin", BRANCH]);
    (dir, work)
}

fn read(dir: &Path) -> GitStatus {
    CliGitRepository::new().status(dir).expect("read status")
}

fn change<'a>(status: &'a GitStatus, path: &str) -> &'a FileChange {
    status
        .changes
        .iter()
        .find(|change| change.path == path)
        .unwrap_or_else(|| panic!("no change reported for {path}: {:?}", status.changes))
}

#[test]
fn a_clean_repository_reports_its_branch_and_nothing_changed() {
    let repo = repository_with(&["a.txt"]);

    let status = read(repo.path());

    assert_eq!(status.branch.name.as_deref(), Some(BRANCH));
    assert_eq!(status.branch.upstream, None);
    assert_eq!(status.branch.sync, SyncState::Unknown);
    assert!(status.changes.is_empty(), "{:?}", status.changes);
}

#[test]
fn every_kind_of_change_is_classified_as_version_control_reports_it() {
    let repo = repository_with(&["modified.txt", "deleted.txt", "moved.txt"]);
    let dir = repo.path();
    write(dir, "modified.txt", "changed\n");
    git(dir, &["rm", "-q", "deleted.txt"]);
    write(dir, "added.txt", "new\n");
    git(dir, &["add", "added.txt"]);
    git(dir, &["mv", "moved.txt", "elsewhere.txt"]);
    write(dir, "untracked.txt", "loose\n");

    let status = read(dir);

    let modified = change(&status, "modified.txt");
    assert_eq!(modified.status.staged, None);
    assert_eq!(modified.status.unstaged, Some(ChangeKind::Modified));

    assert_eq!(
        change(&status, "deleted.txt").status.staged,
        Some(ChangeKind::Deleted)
    );
    assert_eq!(
        change(&status, "added.txt").status.staged,
        Some(ChangeKind::Added)
    );

    let moved = change(&status, "elsewhere.txt");
    assert_eq!(moved.status.staged, Some(ChangeKind::Renamed));
    assert_eq!(moved.original_path.as_deref(), Some("moved.txt"));

    let untracked = change(&status, "untracked.txt");
    assert_eq!(untracked.status.unstaged, Some(ChangeKind::Untracked));
    assert_eq!(untracked.status.staged, None);
}

#[test]
fn nested_untracked_files_are_reported_as_files_instead_of_one_directory_entry() {
    let repo = repository_with(&["tracked.txt"]);
    write(repo.path(), "notes/drafts/first.md", "one\n");
    write(repo.path(), "notes/second.md", "two\n");

    let status = read(repo.path());
    let paths: Vec<&str> = status
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect();

    assert_eq!(
        paths,
        vec!["notes/drafts/first.md", "notes/second.md"],
        "an untracked folder must be expandable from its actual file entries",
    );
}

#[test]
fn status_totals_tracked_and_visible_untracked_text_lines_but_not_binary_bytes() {
    let repo = repository_with(&[
        "staged.txt",
        "unstaged.txt",
        "deleted.txt",
        "tracked-binary.dat",
    ]);
    let dir = repo.path();
    write(dir, "staged.txt", "staged replacement\nand another\n");
    git(dir, &["add", "staged.txt"]);
    write(dir, "unstaged.txt", "unstaged replacement\n");
    std::fs::remove_file(dir.join("deleted.txt")).expect("delete tracked file");
    write(dir, "tracked-binary.dat", "\0changed binary bytes\n");
    write(dir, "notes/draft.md", "first\nsecond");
    write(dir, "assets/new.bin", "\0untracked binary bytes\n");

    let status = read(dir);

    assert_eq!(status.line_counts().additions, 5);
    assert_eq!(status.line_counts().deletions, 3);
    assert!(status.line_counts().complete);
    assert_eq!(
        change(&status, "tracked-binary.dat").status.unstaged,
        Some(ChangeKind::Modified),
        "a binary change remains in the changed-file projection",
    );
    assert_eq!(
        change(&status, "assets/new.bin").status.unstaged,
        Some(ChangeKind::Untracked),
        "an untracked binary remains visible even though it adds no text lines",
    );
}

#[test]
fn a_repository_without_a_head_still_reports_untracked_lines() {
    let repo = tempfile::tempdir().expect("temp dir");
    git(repo.path(), &["init", "-b", BRANCH]);
    write(repo.path(), "draft.txt", "first\nsecond\n");

    let status = read(repo.path());

    assert_eq!(status.line_counts().additions, 2);
    assert_eq!(status.line_counts().deletions, 0);
    assert!(status.line_counts().complete);
    assert_eq!(
        change(&status, "draft.txt").status.unstaged,
        Some(ChangeKind::Untracked),
    );
}

#[test]
fn a_staged_first_commit_file_is_counted_against_an_empty_tree() {
    let repo = tempfile::tempdir().expect("temp dir");
    git(repo.path(), &["init", "-b", BRANCH]);
    write(repo.path(), "first.txt", "first\nsecond\n");
    git(repo.path(), &["add", "first.txt"]);

    let status = read(repo.path());

    assert_eq!(status.line_counts().additions, 2);
    assert_eq!(status.line_counts().deletions, 0);
    assert!(status.line_counts().complete);
    assert_eq!(
        change(&status, "first.txt").status.staged,
        Some(ChangeKind::Added),
    );
}

#[test]
fn an_oversized_untracked_text_file_stays_visible_and_marks_line_totals_incomplete() {
    let repo = repository_with(&["tracked.txt"]);
    std::fs::write(
        repo.path().join("large.txt"),
        vec![b'x'; OVERSIZED_UNTRACKED_TEXT_BYTES],
    )
    .expect("write large file");

    let status = read(repo.path());

    assert_eq!(status.line_counts().additions, 0);
    assert_eq!(status.line_counts().deletions, 0);
    assert!(!status.line_counts().complete);
    assert_eq!(
        change(&status, "large.txt").status.unstaged,
        Some(ChangeKind::Untracked),
    );
}

#[test]
fn a_path_changed_on_both_sides_reports_each_side_separately() {
    let repo = repository_with(&["a.txt"]);
    let dir = repo.path();
    write(dir, "a.txt", "staged\n");
    git(dir, &["add", "a.txt"]);
    write(dir, "a.txt", "staged, then changed again\n");

    let status = read(dir);

    let both = change(&status, "a.txt");
    assert_eq!(both.status.staged, Some(ChangeKind::Modified));
    assert_eq!(both.status.unstaged, Some(ChangeKind::Modified));
}

#[test]
fn a_branch_reports_how_far_it_stands_from_its_upstream() {
    let (_fixture, work) = clone_of_a_remote();

    let pushed = read(&work);
    assert_eq!(pushed.branch.upstream.as_deref(), Some("origin/main"));
    assert_eq!(pushed.branch.sync, SyncState::UpToDate);

    write(&work, "a.txt", "two\n");
    git(&work, &["commit", "-qam", "two"]);
    write(&work, "a.txt", "three\n");
    git(&work, &["commit", "-qam", "three"]);
    assert_eq!(read(&work).branch.sync, SyncState::Ahead { ahead: 2 });

    git(&work, &["push", "-q"]);
    git(&work, &["reset", "-q", "--hard", "HEAD~1"]);
    assert_eq!(read(&work).branch.sync, SyncState::Behind { behind: 1 });
}

#[test]
fn a_path_left_unresolved_by_a_merge_is_reported_as_needing_resolution() {
    let repo = repository_with(&["a.txt"]);
    let dir = repo.path();
    git(dir, &["checkout", "-q", "-b", "other"]);
    write(dir, "a.txt", "theirs\n");
    git(dir, &["commit", "-qam", "theirs"]);
    git(dir, &["checkout", "-q", BRANCH]);
    write(dir, "a.txt", "ours\n");
    git(dir, &["commit", "-qam", "ours"]);
    assert!(
        !try_git(dir, &["merge", "-q", "other"]),
        "the fixture needs a merge that leaves a conflict behind",
    );

    let status = read(dir);

    assert_eq!(
        change(&status, "a.txt").status.unstaged,
        Some(ChangeKind::Conflicted)
    );
}

#[test]
fn a_detached_head_reports_no_branch() {
    let repo = repository_with(&["a.txt"]);
    git(repo.path(), &["checkout", "-q", "--detach", "HEAD"]);

    assert_eq!(read(repo.path()).branch.name, None);
}

#[test]
fn a_folder_that_is_not_a_repository_is_reported_as_such() {
    let dir = tempfile::tempdir().expect("temp dir");

    assert!(matches!(
        CliGitRepository::new().status(dir.path()),
        Err(GitError::NotARepo),
    ));
}
