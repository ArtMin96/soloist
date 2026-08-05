//! The adapter read against real repositories, each built in a temporary directory by the same
//! `git` a user would run. Nothing here is a recording: the fixtures are made with real commits,
//! renames, merges, and a real remote, so what is asserted is what the installed tool actually
//! reports.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use soloist_core::{ChangeKind, FileChange, GitError, GitRepository, GitStatus, SyncState};
use soloist_git::CliGitRepository;
use tempfile::TempDir;

/// The branch every fixture starts on, named rather than inherited so the assertions do not
/// depend on whichever default the host's git was built with.
const BRANCH: &str = "main";

/// Runs `git args` in `dir` under an identity of its own and with no user or system
/// configuration, so a fixture is the same repository on every machine. Returns whether it
/// succeeded, for the invocations a fixture *wants* to fail.
fn try_git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run git")
        .success()
}

/// Runs `git args` in `dir`, failing the test if the fixture step did not take.
fn git(dir: &Path, args: &[&str]) {
    assert!(
        try_git(dir, args),
        "git {args:?} failed in {}",
        dir.display()
    );
}

fn write(dir: &Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).expect("write file");
}

/// A repository with one commit holding each of `files`. Each file gets contents of its own:
/// identical files are interchangeable to rename detection, which would pair a deletion with an
/// unrelated addition and make the fixture describe something other than what it set up.
fn repository_with(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-b", BRANCH]);
    for file in files {
        write(
            dir.path(),
            file,
            &format!("the original contents of {file}\n"),
        );
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    dir
}

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
