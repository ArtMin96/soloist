//! What listing, switching, creating and deleting branches — and the stash — do to a real
//! repository, asserted against version control's own account of it rather than against the
//! adapter's.

mod fixture;

use fixture::{commit, git, git_output, porcelain_status, repository_with, write, BRANCH};

use soloist_core::{BranchOp, GitError, GitRepository, StashOp};
use soloist_git::CliGitRepository;

/// The listing bound every test here reads under — larger than any fixture, so nothing is left out
/// by the cap when a test is about something else.
const EVERY_BRANCH: usize = 50;

#[test]
fn every_branch_is_listed_with_the_checked_out_one_marked() {
    let dir = repository_with(&["a.txt"]);
    git(dir.path(), &["branch", "feature"]);

    let branches = CliGitRepository::new()
        .branches(dir.path(), EVERY_BRANCH)
        .expect("branches");

    let names: Vec<&str> = branches
        .entries
        .iter()
        .map(|branch| branch.name.as_str())
        .collect();
    assert!(
        names.contains(&BRANCH) && names.contains(&"feature"),
        "{names:?}"
    );
    let head: Vec<&str> = branches
        .entries
        .iter()
        .filter(|branch| branch.head)
        .map(|branch| branch.name.as_str())
        .collect();
    assert_eq!(head, vec![BRANCH], "exactly one branch is checked out");
}

#[test]
fn a_listing_carries_no_more_branches_than_it_was_asked_for() {
    let dir = repository_with(&["a.txt"]);
    for index in 0..5 {
        git(dir.path(), &["branch", &format!("feature-{index}")]);
    }

    let branches = CliGitRepository::new()
        .branches(dir.path(), 2)
        .expect("branches");

    assert_eq!(branches.entries.len(), 2);
}

#[test]
fn a_folder_outside_any_repository_has_no_branches_to_list() {
    let dir = tempfile::tempdir().expect("temp dir");

    let refusal = CliGitRepository::new()
        .branches(dir.path(), EVERY_BRANCH)
        .unwrap_err();

    assert_eq!(refusal, GitError::NotARepo);
}

#[test]
fn creating_a_branch_starts_it_where_the_working_tree_is_and_switches_to_it() {
    let dir = repository_with(&["a.txt"]);

    CliGitRepository::new()
        .branch(dir.path(), BranchOp::Create, "feature")
        .expect("create");

    assert_eq!(
        git_output(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "feature",
    );
    assert_eq!(
        git_output(dir.path(), &["rev-parse", "feature"]),
        git_output(dir.path(), &["rev-parse", BRANCH]),
        "a new branch starts at the commit that was checked out",
    );
}

#[test]
fn switching_puts_the_other_branch_in_the_working_tree() {
    let dir = repository_with(&["a.txt"]);
    git(dir.path(), &["switch", "--create", "feature"]);
    write(dir.path(), "only-on-feature.txt", "here\n");
    commit(dir.path(), "add a file on the feature branch");
    git(dir.path(), &["switch", BRANCH]);

    CliGitRepository::new()
        .branch(dir.path(), BranchOp::Switch, "feature")
        .expect("switch");

    assert!(dir.path().join("only-on-feature.txt").exists());
}

#[test]
fn a_switch_that_would_overwrite_uncommitted_work_is_refused_in_version_controls_own_words() {
    let dir = repository_with(&["a.txt"]);
    git(dir.path(), &["switch", "--create", "feature"]);
    write(dir.path(), "a.txt", "the feature's version\n");
    commit(dir.path(), "change the file on the feature branch");
    git(dir.path(), &["switch", BRANCH]);
    write(dir.path(), "a.txt", "work in progress\n");

    let refusal = CliGitRepository::new()
        .branch(dir.path(), BranchOp::Switch, "feature")
        .unwrap_err();

    let GitError::Refused { output } = refusal else {
        panic!("expected version control's own account of what is in the way: {refusal:?}");
    };
    assert!(output.contains("a.txt"), "{output}");
    assert_eq!(
        git_output(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        BRANCH,
        "nothing moved",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        "work in progress\n",
        "and the work that was in the way is still there — nothing stashed it away",
    );
}

#[test]
fn deleting_a_branch_whose_work_is_already_merged_removes_it() {
    let dir = repository_with(&["a.txt"]);
    git(dir.path(), &["branch", "feature"]);

    CliGitRepository::new()
        .branch(dir.path(), BranchOp::Delete, "feature")
        .expect("delete");

    assert!(!git_output(dir.path(), &["branch", "--list"]).contains("feature"));
}

#[test]
fn deleting_a_branch_holding_commits_nothing_else_holds_stays_refused() {
    let dir = repository_with(&["a.txt"]);
    git(dir.path(), &["switch", "--create", "feature"]);
    write(dir.path(), "b.txt", "only here\n");
    commit(dir.path(), "work only this branch holds");
    git(dir.path(), &["switch", BRANCH]);

    let refusal = CliGitRepository::new()
        .branch(dir.path(), BranchOp::Delete, "feature")
        .unwrap_err();

    let GitError::Refused { output } = refusal else {
        panic!("expected version control's own reason: {refusal:?}");
    };
    assert!(output.contains("not fully merged"), "{output}");
    assert!(
        git_output(dir.path(), &["branch", "--list"]).contains("feature"),
        "there is no forced delete anywhere, so the branch is still there",
    );
}

#[test]
fn stashing_leaves_the_working_tree_as_the_last_commit_left_it_and_taking_it_back_restores_it() {
    let dir = repository_with(&["a.txt"]);
    let original = std::fs::read_to_string(dir.path().join("a.txt")).expect("read");
    write(dir.path(), "a.txt", "changed\n");
    let repository = CliGitRepository::new();

    repository.stash(dir.path(), StashOp::Save).expect("stash");

    assert_eq!(porcelain_status(dir.path(), "a.txt"), "");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        original,
    );
    assert!(
        repository
            .branches(dir.path(), EVERY_BRANCH)
            .expect("branches")
            .stashed,
        "something is set aside, which is what makes taking it back an action at all",
    );

    repository.stash(dir.path(), StashOp::Pop).expect("pop");

    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        "changed\n",
    );
    assert!(
        !repository
            .branches(dir.path(), EVERY_BRANCH)
            .expect("branches")
            .stashed,
        "and taking it back leaves nothing behind",
    );
}

#[test]
fn stashing_leaves_a_file_version_control_does_not_track_where_it_is() {
    let dir = repository_with(&["a.txt"]);
    write(dir.path(), "a.txt", "changed\n");
    write(dir.path(), "notes.md", "mine\n");

    CliGitRepository::new()
        .stash(dir.path(), StashOp::Save)
        .expect("stash");

    assert!(
        dir.path().join("notes.md").exists(),
        "taking an untracked file would mean a file the user made disappearing",
    );
}

#[test]
fn taking_stashed_changes_back_over_a_collision_reports_it_rather_than_claiming_success() {
    let dir = repository_with(&["a.txt"]);
    write(dir.path(), "a.txt", "the stashed version\n");
    let repository = CliGitRepository::new();
    repository.stash(dir.path(), StashOp::Save).expect("stash");
    write(dir.path(), "a.txt", "a committed version that collides\n");
    commit(dir.path(), "move the file on underneath the stash");

    let refusal = repository.stash(dir.path(), StashOp::Pop).unwrap_err();

    let GitError::Refused { output } = refusal else {
        panic!("expected version control's own account of the collision: {refusal:?}");
    };
    assert!(
        output.contains("CONFLICT") || output.contains("conflict"),
        "{output}"
    );
    assert!(
        repository
            .branches(dir.path(), EVERY_BRANCH)
            .expect("branches")
            .stashed,
        "what was set aside is kept, which the reader can only learn from the report",
    );
}
