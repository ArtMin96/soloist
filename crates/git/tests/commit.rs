//! Committing in a real repository, built by the same `git` a user would run.
//!
//! The point of driving the command line is that the repository's own hooks and the user's own
//! configuration apply without a line of code here, so those are what these assert: a hook runs,
//! a hook that refuses stops the commit and its words come back, and an amend rewrites what is
//! committed without touching the working tree.

use std::path::Path;

use soloist_core::{GitError, GitRepository};
use soloist_git::CliGitRepository;

mod fixture;
use fixture::{git, git_output, repository_with, write};

const PATH: &str = "notes.md";

const MESSAGE: &str = "Record the change";

/// Installs `script` as `name`, which version control runs at the point that name reserves.
fn hook(dir: &Path, name: &str, script: &str) {
    let path = dir.join(".git/hooks").join(name);
    std::fs::create_dir_all(path.parent().expect("hooks directory")).expect("create hooks");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

/// A repository with one change staged, ready to be committed.
fn with_a_staged_change() -> tempfile::TempDir {
    let dir = repository_with(&["placeholder"]);
    write(dir.path(), PATH, "the first line\n");
    git(dir.path(), &["add", PATH]);
    dir
}

/// The subjects of every commit on the branch, newest first.
fn subjects(dir: &Path) -> Vec<String> {
    git_output(dir, &["log", "--format=%s"])
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_commit_records_the_index_under_the_message_it_was_given() {
    let dir = with_a_staged_change();

    CliGitRepository::new()
        .commit(dir.path(), MESSAGE, false)
        .expect("commit");

    assert_eq!(
        subjects(dir.path()).first().map(String::as_str),
        Some(MESSAGE)
    );
    assert_eq!(
        git_output(dir.path(), &["status", "--porcelain"]),
        "",
        "everything staged went into the commit",
    );
}

#[test]
fn the_repositorys_own_pre_commit_hook_runs() {
    let dir = with_a_staged_change();
    hook(dir.path(), "pre-commit", "echo ran > hook-ran.txt");

    CliGitRepository::new()
        .commit(dir.path(), MESSAGE, false)
        .expect("commit");

    assert!(
        dir.path().join("hook-ran.txt").exists(),
        "the user's hooks apply because it is the user's own git that runs",
    );
}

#[test]
fn a_pre_commit_hook_that_refuses_stops_the_commit_and_its_own_words_come_back() {
    let dir = with_a_staged_change();
    hook(
        dir.path(),
        "pre-commit",
        "echo 'the tests are red' >&2; exit 1",
    );

    let refusal = CliGitRepository::new()
        .commit(dir.path(), MESSAGE, false)
        .unwrap_err();

    match refusal {
        GitError::Refused { output } => assert!(
            output.contains("the tests are red"),
            "a rejected commit that says nothing is one nobody can act on: {output}",
        ),
        other => panic!("expected a refusal carrying the hook's words, got {other:?}"),
    }
    assert_eq!(subjects(dir.path()), vec!["start"], "nothing was committed");
    assert_eq!(
        git_output(dir.path(), &["status", "--porcelain"]),
        "A  notes.md\n",
        "and the index is exactly as it was left",
    );
}

#[test]
fn a_commit_msg_hook_that_refuses_is_reported_the_same_way() {
    let dir = with_a_staged_change();
    hook(dir.path(), "commit-msg", "echo 'say why' >&2; exit 1");

    let refusal = CliGitRepository::new()
        .commit(dir.path(), MESSAGE, false)
        .unwrap_err();

    assert!(
        matches!(&refusal, GitError::Refused { output } if output.contains("say why")),
        "{refusal:?}",
    );
}

#[test]
fn an_amend_rewrites_the_last_commit_rather_than_adding_one() {
    let dir = with_a_staged_change();
    let repository = CliGitRepository::new();
    repository
        .commit(dir.path(), MESSAGE, false)
        .expect("commit");
    write(dir.path(), "unrelated.md", "not staged\n");

    repository
        .commit(dir.path(), "A better message", true)
        .expect("amend");

    assert_eq!(
        subjects(dir.path()),
        vec!["A better message", "start"],
        "the tip was replaced, not added to",
    );
    assert_eq!(
        git_output(dir.path(), &["status", "--porcelain"]),
        "?? unrelated.md\n",
        "and the working tree is exactly as the amend found it",
    );
}

#[test]
fn a_multi_line_message_keeps_its_subject_and_its_body() {
    let dir = with_a_staged_change();

    CliGitRepository::new()
        .commit(dir.path(), "A subject\n\nA body that explains why.", false)
        .expect("commit");

    assert_eq!(
        git_output(dir.path(), &["log", "-1", "--format=%s%n%n%b"]),
        "A subject\n\nA body that explains why.\n\n",
    );
}
