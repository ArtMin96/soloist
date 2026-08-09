//! Behavioural tests for what an agent is told about a branch it is asked to describe. They assert
//! the composed prompt, which is the whole of what the agent sees — nothing here runs one.

use crate::git::NoopFileOpener;
use std::path::Path;
use std::sync::Arc;

use crate::agents::ONE_SHOT_PROMPT_LIMIT;
use crate::ids::ProjectId;
use crate::testing::{commit_entry, git_status, FakeGitForge, FakeGitRepository, FakeTrustRepo};

use super::{Git, PullRequestError};

/// The fakes ignore it — everything here is addressed by project, not by path.
const ROOT: &str = "/project";

const BRANCH: &str = "feature";
const BASE: &str = "main";

const SKELETON: &str = "## What changed\n\n## Why\n";

/// The git context over `repository`, with `project` trusted — drafting is gated like a change.
fn trusting(repository: FakeGitRepository, project: ProjectId) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(FakeGitForge::ready()),
        Arc::new(NoopFileOpener),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
    ))
}

/// A repository on `BRANCH` whose branch proposes `subjects` beyond its base.
fn proposing(subjects: &[&str]) -> FakeGitRepository {
    FakeGitRepository::reporting(git_status(BRANCH)).proposing(
        subjects
            .iter()
            .enumerate()
            .map(|(index, subject)| commit_entry(&format!("{index:040}"), subject, "Somebody"))
            .collect(),
    )
}

#[test]
fn a_branch_holding_nothing_its_base_does_not_has_nothing_to_describe() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH)).proposing(Vec::new());
    let project = ProjectId::next();
    let git = trusting(repository, project);

    let refusal = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, SKELETON)
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::NothingToDescribe),
        "{refusal:?}",
    );
}

#[test]
fn a_project_that_has_not_been_trusted_composes_nothing_to_ask_an_agent() {
    let repository = proposing(&["Add the thing"]);
    let project = ProjectId::next();
    let git = Arc::new(Git::new(
        Arc::new(repository.clone()),
        Arc::new(FakeGitForge::ready()),
        Arc::new(NoopFileOpener),
        Arc::new(FakeTrustRepo::new()),
    ));

    let refusal = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, SKELETON)
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::Untrusted),
        "{refusal:?}"
    );
    assert_eq!(
        repository.reads(),
        0,
        "an agent CLI reads the project's own configuration, so the gate is spent before the \
         repository is even read",
    );
}

#[test]
fn a_base_version_control_would_read_as_an_option_is_refused_rather_than_ranged_against() {
    let repository = proposing(&["Add the thing"]);
    let project = ProjectId::next();
    let git = trusting(repository.clone(), project);

    let refusal = git
        .pull_request_prompt(project, Path::new(ROOT), "--output=/tmp/hack", SKELETON)
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::UnusableBranchName),
        "{refusal:?}",
    );
    assert_eq!(repository.reads(), 0);
}

#[test]
fn the_prompt_names_the_branch_and_the_one_it_would_merge_into() {
    let project = ProjectId::next();
    let git = trusting(proposing(&["Add the thing"]), project);

    let prompt = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, SKELETON)
        .expect("prompt");

    assert!(
        prompt.contains(&format!("Merging branch {BRANCH} into {BASE}")),
        "a branch and its base are often the only place a change's purpose is named: {prompt}",
    );
}

#[test]
fn the_prompt_lists_what_the_branch_proposes_rather_than_its_whole_history() {
    let repository = proposing(&["Add the thing"]).logging(vec![commit_entry(
        "9",
        "Something from long before this branch",
        "Somebody",
    )]);
    let project = ProjectId::next();
    let git = trusting(repository, project);

    let prompt = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, SKELETON)
        .expect("prompt");

    assert!(prompt.contains("- Add the thing"), "{prompt}");
    assert!(
        !prompt.contains("long before this branch"),
        "a base merged in along the way is not part of what is being proposed: {prompt}",
    );
}

#[test]
fn the_shape_to_fill_is_the_last_thing_the_prompt_says() {
    let project = ProjectId::next();
    let git = trusting(proposing(&["Add the thing"]), project);

    let prompt = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, SKELETON)
        .expect("prompt");

    assert!(
        prompt.ends_with(SKELETON),
        "the final thing read has to be the form the answer must take: {prompt}",
    );
    assert!(
        prompt.contains("Keep the template's headings"),
        "a repository's convention is the contract, so the instructions say to keep it: {prompt}",
    );
}

#[test]
fn a_skeleton_too_long_to_be_a_shape_is_dropped_rather_than_handed_over_in_pieces() {
    let project = ProjectId::next();
    let git = trusting(proposing(&["Add the thing"]), project);
    let enormous = "## Section\n".repeat(2_000);

    let prompt = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, &enormous)
        .expect("prompt");

    assert!(
        !prompt.contains("## Section"),
        "half a skeleton would be filled in as if it were the whole of one: {}",
        prompt.len(),
    );
    assert!(
        prompt.contains("Say what it changes and why it changes it"),
        "with no shape to fill, the answer's form is the agent's own: {prompt}",
    );
}

#[test]
fn a_branch_with_more_commits_than_fit_is_composed_to_the_ceiling_rather_than_cut_to_it() {
    let subjects: Vec<String> = (0..60)
        .map(|index| format!("Do the {index} thing {}", "x".repeat(880)))
        .collect();
    let borrowed: Vec<&str> = subjects.iter().map(String::as_str).collect();
    let project = ProjectId::next();
    let git = trusting(proposing(&borrowed), project);
    let skeleton = "## Section\n".repeat(700);

    let prompt = git
        .pull_request_prompt(project, Path::new(ROOT), BASE, &skeleton)
        .expect("prompt");

    assert!(
        prompt.len() <= ONE_SHOT_PROMPT_LIMIT,
        "the prompt is composed to the ceiling: {}",
        prompt.len(),
    );
    assert!(
        prompt.ends_with(&skeleton),
        "the shape is what the room is kept for, so it survives whole while commits are dropped",
    );
    assert!(
        prompt.contains("- Do the 0 thing"),
        "the newest commits are what the branch most recently became, so they are kept",
    );
    assert!(
        !prompt.contains("- Do the 49 thing"),
        "and the oldest are what is dropped when the room runs out",
    );
}
