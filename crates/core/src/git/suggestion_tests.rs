//! Behavioural tests for what a pull request would be opened with before anybody types anything.
//! They assert the computed title and description, which is the whole of what a one-click proposal
//! is made of.

use std::path::Path;
use std::sync::Arc;

use crate::git::{NoopFileOpener, PullRequestError};
use crate::ids::ProjectId;
use crate::testing::{
    commit_entry, described_entry, git_status, FakeGitForge, FakeGitRepository, FakeTrustRepo,
};
use crate::vcs::CommitEntry;

use super::{Git, PullRequestSuggestion, TITLE_LIMIT};

/// The fakes ignore it — everything here is addressed by project, not by path.
const ROOT: &str = "/project";

const BRANCH: &str = "feat/live-changes-rail";
const BASE: &str = "main";

/// A repository's own shape, headings and checklist and all.
const SKELETON: &str = "## What changed\n\n## Why\n\n- [ ] Tests\n";

/// The git context over `repository`, with nothing trusted: computing a suggestion reads the log and
/// runs nothing the repository configures, so it is ungated like every other read.
fn context(repository: FakeGitRepository) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(FakeGitForge::ready()),
        Arc::new(NoopFileOpener),
        Arc::new(FakeTrustRepo::new()),
    ))
}

/// A repository on `branch` proposing `commits` beyond its base.
fn proposing(branch: &str, commits: Vec<CommitEntry>) -> FakeGitRepository {
    FakeGitRepository::reporting(git_status(branch)).proposing(commits)
}

/// What `branch` would be proposed as, carrying `commits` beyond its base, with `skeleton` on offer.
fn suggested(branch: &str, commits: Vec<CommitEntry>, skeleton: &str) -> PullRequestSuggestion {
    context(proposing(branch, commits))
        .pull_request_suggestion(ProjectId::next(), Path::new(ROOT), BASE, skeleton)
        .expect("suggestion")
}

/// Two commits, newest first — a branch with no single account of itself.
fn two_commits() -> Vec<CommitEntry> {
    vec![
        commit_entry("1", "Render the rail", "Somebody"),
        commit_entry("0", "Read the working tree", "Somebody"),
    ]
}

#[test]
fn a_branch_carrying_one_commit_is_proposed_as_what_that_commit_says() {
    let suggestion = suggested(
        BRANCH,
        vec![described_entry(
            "0",
            "Add the live changes rail",
            "It reads the working tree once and renders what changed.",
        )],
        "",
    );

    assert_eq!(
        suggestion.title, "Add the live changes rail",
        "the commit has already been described once, by whoever wrote it",
    );
    assert_eq!(
        suggestion.body,
        "It reads the working tree once and renders what changed.",
    );
}

#[test]
fn a_branch_carrying_several_commits_is_proposed_as_its_name_and_their_subjects() {
    let suggestion = suggested(BRANCH, two_commits(), "");

    assert_eq!(suggestion.title, "Live changes rail");
    assert_eq!(
        suggestion.body, "- Render the rail\n- Read the working tree",
        "newest first, as every other account of what a branch proposes is ordered",
    );
}

#[test]
fn a_branch_holding_nothing_its_base_does_not_has_nothing_to_suggest() {
    let git = context(proposing(BRANCH, Vec::new()));

    let refusal = git
        .pull_request_suggestion(ProjectId::next(), Path::new(ROOT), BASE, "")
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::NothingToDescribe),
        "no title can be computed from no commits, and inventing one would be worse: {refusal:?}",
    );
}

#[test]
fn a_repositorys_own_shape_is_filled_in_rather_than_replaced() {
    let suggestion = suggested(BRANCH, two_commits(), SKELETON);

    assert_eq!(
        suggestion.body,
        "## What changed\n\n- Render the rail\n- Read the working tree\n\n## Why\n\n- [ ] Tests\n",
        "a repository carrying a template is saying what it expects to read, so every heading and \
         its checklist survive with the branch's own account written under the first of them",
    );
}

#[test]
fn a_shape_with_no_heading_to_write_under_takes_the_account_above_it() {
    let suggestion = suggested(BRANCH, two_commits(), "Thanks for the patch!\n");

    assert_eq!(
        suggestion.body,
        "- Render the rail\n- Read the working tree\n\nThanks for the patch!\n",
    );
}

#[test]
fn a_branch_name_stands_in_for_a_title_as_a_sentence() {
    for (branch, title) in [
        ("feat/live-changes-rail", "Live changes rail"),
        ("chore-bump-the-deps", "Bump the deps"),
        ("fix_the_login_redirect", "The login redirect"),
        ("live-changes-rail", "Live changes rail"),
        ("Fix the login redirect", "Fix the login redirect"),
    ] {
        let suggestion = suggested(branch, two_commits(), "");

        assert_eq!(suggestion.title, title, "from {branch}");
    }
}

#[test]
fn how_a_branch_spells_its_kind_of_work_makes_no_difference_to_the_title() {
    // `/`, `-` and `_` are three ways of writing the same name, so a title computed from one has to
    // read the same as a title computed from another — the alternative is that `fix/login` and
    // `fix_login` disagree about whether "fix" is part of what changed.
    let titles: Vec<String> = [
        "fix/the-login-redirect",
        "fix-the-login-redirect",
        "fix_the_login_redirect",
    ]
    .into_iter()
    .map(|branch| suggested(branch, two_commits(), "").title)
    .collect();

    assert_eq!(
        titles,
        vec![
            "The login redirect".to_string(),
            "The login redirect".to_string(),
            "The login redirect".to_string(),
        ],
    );
}

#[test]
fn a_subject_that_is_really_a_whole_paragraph_is_carried_only_as_far_as_a_title_goes() {
    // What `%s` gives for a commit whose author wrote no blank line after the first line: the whole
    // first paragraph, folded into one line. A title is a line, so it cannot be all of that.
    let rambled = "Add the live changes rail ".repeat(80);
    let suggestion = suggested(
        BRANCH,
        vec![described_entry("0", rambled.trim_end(), "")],
        "",
    );

    assert!(
        suggestion.title.len() <= TITLE_LIMIT,
        "a title is bounded: {} bytes",
        suggestion.title.len(),
    );
    assert!(
        !suggestion.title.is_empty() && rambled.starts_with(&suggestion.title),
        "and it is the opening of what the author wrote, whole words: {:?}",
        suggestion.title,
    );
    assert!(
        !suggestion.title.ends_with("rai") && !suggestion.title.ends_with(' '),
        "cut at a word boundary, not mid-word: {:?}",
        suggestion.title,
    );
}
