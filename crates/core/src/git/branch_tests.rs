//! Behavioural tests for what is checked out, what else could be, and the stash — driving a real
//! [`Git`] over the shared [`FakeGitRepository`], so what is asserted is what a caller observes:
//! either the repository was asked to do something, or it was never reached at all.

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::ids::ProjectId;
use crate::testing::{
    branch, branches, git_over, git_status, git_trusting, FakeGitRepository, GitChange,
};

use super::{BranchOp, Git, GitWriteError, StashOp, BRANCH_PAGE_SIZE};

/// The fake ignores it — everything here is addressed by project, not by path.
const ROOT: &str = "/project";

const FEATURE: &str = "feature";

/// Every operation that names a branch, so a rule that has to hold for all three is stated once.
fn every_named_change(git: &Git, project: ProjectId, name: &str) -> Vec<GitWriteError> {
    let root = Path::new(ROOT);
    vec![
        git.create_branch(project, root, name).unwrap_err(),
        git.switch_branch(project, root, name).unwrap_err(),
        git.delete_branch(project, root, name).unwrap_err(),
    ]
}

/// Every way of changing which branch is checked out or where the working tree's changes are, so a
/// rule that has to hold for all of them is stated once rather than five times.
fn every_change(git: &Git, project: ProjectId, name: &str) -> Vec<GitWriteError> {
    let root = Path::new(ROOT);
    let mut refusals = every_named_change(git, project, name);
    refusals.push(git.stash(project, root).unwrap_err());
    refusals.push(git.pop_stash(project, root).unwrap_err());
    refusals
}

#[test]
fn a_project_that_has_not_been_trusted_can_neither_switch_branch_nor_stash() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let git = git_over(repository.clone());
    let project = ProjectId::next();

    let refusals = every_change(&git, project, FEATURE);

    assert!(
        refusals
            .iter()
            .all(|refusal| matches!(refusal, GitWriteError::Untrusted)),
        "changing which branch is checked out changes the working tree, so it passes the same \
         gate every other change does: {refusals:?}",
    );
    assert_eq!(
        repository.changes(),
        Vec::new(),
        "a refused change never reaches the repository at all",
    );
}

#[test]
fn each_branch_operation_asks_for_the_one_it_names() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);
    let root = Path::new(ROOT);

    git.create_branch(project, root, FEATURE).expect("create");
    git.switch_branch(project, root, "main").expect("switch");
    git.delete_branch(project, root, FEATURE).expect("delete");

    assert_eq!(
        repository.changes(),
        vec![
            GitChange::Branch {
                op: BranchOp::Create,
                name: FEATURE.to_string(),
            },
            GitChange::Branch {
                op: BranchOp::Switch,
                name: "main".to_string(),
            },
            GitChange::Branch {
                op: BranchOp::Delete,
                name: FEATURE.to_string(),
            },
        ],
    );
}

#[test]
fn a_name_version_control_would_read_as_an_option_never_reaches_it() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let refusals: Vec<GitWriteError> = ["--upload-pack=/bin/sh", "   "]
        .into_iter()
        .flat_map(|name| every_named_change(&git, project, name))
        .collect();

    assert!(
        refusals
            .iter()
            .all(|refusal| matches!(refusal, GitWriteError::UnusableBranchName)),
        "an argument is not a place to accept arbitrary text: {refusals:?}",
    );
    assert_eq!(
        repository.changes(),
        Vec::new(),
        "the guard is spent before anything runs",
    );
}

#[test]
fn stashing_and_taking_it_back_are_the_two_directions_of_one_operation() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.stash(project, Path::new(ROOT)).expect("stash");
    git.pop_stash(project, Path::new(ROOT)).expect("pop");

    assert_eq!(
        repository.changes(),
        vec![
            GitChange::Stash { op: StashOp::Save },
            GitChange::Stash { op: StashOp::Pop },
        ],
    );
}

#[test]
fn listing_branches_needs_no_trust_because_it_changes_nothing() {
    let listed = branches(vec![branch("main", true), branch(FEATURE, false)]);
    let repository = FakeGitRepository::reporting(git_status("main")).branching(listed.clone());
    let git = git_over(repository);
    let project = ProjectId::next();

    let read = git.branches(project, Path::new(ROOT)).expect("branches");

    assert_eq!(read, Some(listed));
}

#[test]
fn a_root_that_is_not_a_repository_has_no_branches_rather_than_a_failure() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let git = git_over(repository);

    let read = git
        .branches(ProjectId::next(), Path::new(ROOT))
        .expect("branches");

    assert_eq!(
        read, None,
        "a project kept out of version control is a choice"
    );
}

#[test]
fn no_caller_can_ask_for_more_branches_than_one_page_holds() {
    let many = branches(
        (0..BRANCH_PAGE_SIZE + 40)
            .map(|index| branch(&format!("branch-{index}"), false))
            .collect(),
    );
    let repository = FakeGitRepository::reporting(git_status("main")).branching(many);
    let git = git_over(repository);

    let read = git
        .branches(ProjectId::next(), Path::new(ROOT))
        .expect("branches")
        .expect("a repository");

    assert_eq!(read.entries.len(), BRANCH_PAGE_SIZE);
}

#[test]
fn a_branch_listing_and_a_status_never_run_against_one_repository_at_once() {
    let repository = FakeGitRepository::slow(git_status("main"), Duration::from_millis(30))
        .branching(branches(vec![branch("main", true)]));
    let project = ProjectId::next();
    let git: Arc<Git> = git_over(repository.clone());

    let listing = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            git.branches(project, Path::new(ROOT)).ok();
        })
    };
    git.status(project, Path::new(ROOT)).ok();
    listing.join().expect("the listing");

    assert_eq!(
        repository.peak_concurrent(),
        1,
        "a listing costs a subprocess like any other read, so it waits its turn against the same \
         repository rather than starting a rival one",
    );
}

#[test]
fn a_branch_change_and_a_read_never_run_against_one_repository_at_once() {
    let repository = FakeGitRepository::slow(git_status("main"), Duration::from_millis(30));
    let project = ProjectId::next();
    let git: Arc<Git> = git_trusting(repository.clone(), project);

    let reading = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            git.files(project, Path::new(ROOT)).ok();
        })
    };
    git.switch_branch(project, Path::new(ROOT), FEATURE)
        .expect("switch");
    reading.join().expect("read");

    assert_eq!(
        repository.peak_concurrent(),
        1,
        "switching branches rewrites the working tree, so it takes the same per-project gate a \
         read does",
    );
}
