//! Behavioural tests for recording the index as a commit, driving a real [`Git`] over the
//! shared [`FakeGitRepository`]. What is asserted is what a caller observes: the commit the
//! repository was asked for, or the refusal that stopped it before anything ran.

use std::path::Path;

use crate::ids::ProjectId;
use crate::testing::{
    file_change, git_over, git_status, git_trusting, FakeGitRepository, GitChange,
};
use crate::vcs::ChangeKind;

use crate::git::{GitError, GitStatus};

use super::GitWriteError;

/// The fake ignores it — a commit is addressed by project here, not by path.
const ROOT: &str = "/project";

const MESSAGE: &str = "Record the index";

/// A working tree with one path staged, so a commit has something to record.
fn with_something_staged() -> FakeGitRepository {
    let mut status: GitStatus = git_status("main");
    status.changes = vec![file_change("src/main.rs", Some(ChangeKind::Modified), None)];
    FakeGitRepository::reporting(status)
}

/// A working tree whose only change is unstaged, so there is nothing for a commit to record.
fn with_nothing_staged() -> FakeGitRepository {
    let mut status: GitStatus = git_status("main");
    status.changes = vec![file_change("src/main.rs", None, Some(ChangeKind::Modified))];
    FakeGitRepository::reporting(status)
}

#[test]
fn a_commit_carries_the_message_it_was_given() {
    let repository = with_something_staged();
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.commit(project, Path::new(ROOT), MESSAGE, false)
        .expect("commit");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Commit {
            message: MESSAGE.to_string(),
            amend: false,
        }],
    );
}

#[test]
fn a_message_of_nothing_but_blank_space_is_refused_before_anything_runs() {
    let repository = with_something_staged();
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let refusal = git
        .commit(project, Path::new(ROOT), "  \n\t ", false)
        .unwrap_err();

    assert!(
        matches!(refusal, GitWriteError::EmptyMessage),
        "{refusal:?}"
    );
    assert_eq!(repository.changes(), Vec::new());
}

#[test]
fn a_message_is_trimmed_of_the_blank_space_around_it() {
    let repository = with_something_staged();
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.commit(project, Path::new(ROOT), "\n  Record the index\n\n", false)
        .expect("commit");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Commit {
            message: MESSAGE.to_string(),
            amend: false,
        }],
    );
}

#[test]
fn a_first_commit_with_nothing_staged_is_refused_without_spending_an_invocation() {
    let repository = with_nothing_staged();
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let refusal = git
        .commit(project, Path::new(ROOT), MESSAGE, false)
        .unwrap_err();

    assert!(
        matches!(refusal, GitWriteError::NothingStaged),
        "{refusal:?}"
    );
    assert_eq!(repository.changes(), Vec::new());
}

#[test]
fn an_amend_with_nothing_staged_is_allowed_because_it_is_how_a_message_is_corrected() {
    let repository = with_nothing_staged();
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.commit(project, Path::new(ROOT), MESSAGE, true)
        .expect("amend");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Commit {
            message: MESSAGE.to_string(),
            amend: true,
        }],
    );
}

#[test]
fn a_project_that_has_not_been_trusted_cannot_commit() {
    let repository = with_something_staged();
    let git = git_over(repository.clone());

    let refusal = git
        .commit(ProjectId::next(), Path::new(ROOT), MESSAGE, false)
        .unwrap_err();

    assert!(matches!(refusal, GitWriteError::Untrusted), "{refusal:?}");
    assert_eq!(repository.changes(), Vec::new());
}

#[test]
fn a_hook_that_refuses_a_commit_carries_its_own_words_back() {
    let rejection = "pre-commit: the tests are red";
    let repository = with_something_staged().refusing(GitError::Refused {
        output: rejection.to_string(),
    });
    let project = ProjectId::next();
    let git = git_trusting(repository, project);

    let refusal = git
        .commit(project, Path::new(ROOT), MESSAGE, false)
        .unwrap_err();

    assert!(
        matches!(&refusal, GitWriteError::Git(GitError::Refused { output }) if output == rejection),
        "a rejected commit that says nothing is a rejected commit nobody can act on: {refusal:?}",
    );
}
