//! What an agent is told about a staged change, driving a real [`Git`] over the shared
//! [`FakeGitRepository`]. What is asserted is the subject a caller composed — the prompt itself —
//! because that is the whole observable output of composing one.

use std::path::Path;

use crate::agents::ONE_SHOT_PROMPT_LIMIT;
use crate::ids::ProjectId;
use crate::testing::{
    commit_entry, file_change, git_over, git_status, git_trusting, merge_entry, raw_diff,
    FakeGitRepository,
};
use crate::vcs::{ChangeKind, CommitEntry, FileChange};

use crate::git::{GitDraftError, GitStatus};

/// The fake ignores it — a read is addressed by project here, not by path.
const ROOT: &str = "/project";

const HUNK: &str = "@@ -1,1 +1,1 @@\n-old\n+new\n";

/// As many staged paths as are still described by their own patch, and as many recent commits as
/// are ever shown — the widest a prompt's context and change both get at once.
const DESCRIBED_PATHS: usize = 48;
const VOICE_EXAMPLE_COUNT: usize = 10;

/// A working tree with `changes` staged, answering the same patch for every path read.
///
/// It has no history, which is one of the states a real repository can be in and keeps these tests
/// about the change alone. The tests about the voice examples state a history of their own.
fn repository_with(changes: Vec<FileChange>, hunk: &str) -> FakeGitRepository {
    let mut status: GitStatus = git_status("main");
    status.changes = changes;
    FakeGitRepository::reporting(status).diffing(raw_diff("diff --git a/f b/f\n", &[hunk]))
}

/// The same, with one path staged and `commits` behind it.
fn repository_logging(commits: Vec<CommitEntry>) -> FakeGitRepository {
    repository_with(vec![staged("src/main.rs")], HUNK).logging(commits)
}

fn staged(path: &str) -> FileChange {
    file_change(path, Some(ChangeKind::Modified), None)
}

/// A patch long enough that a handful of them pass the prompt ceiling.
fn long_hunk() -> String {
    format!("@@ -1,1 +1,1 @@\n+{}\n", "x".repeat(12 * 1024))
}

fn prompt_for(repository: FakeGitRepository) -> Result<String, GitDraftError> {
    let project = ProjectId::next();
    let git = git_trusting(repository, project);
    git.commit_message_prompt(project, Path::new(ROOT))
}

#[test]
fn a_staged_change_is_described_by_its_own_patch() {
    let prompt = prompt_for(repository_with(vec![staged("src/main.rs")], HUNK)).expect("prompt");

    assert!(
        prompt.contains("-old\n+new\n"),
        "the diff is the subject; without it there is nothing to describe: {prompt}",
    );
    assert!(
        prompt.contains("diff --git a/f b/f"),
        "the patch keeps the header that says which file it belongs to: {prompt}",
    );
}

#[test]
fn only_what_is_staged_is_described() {
    // A message describes the commit that is about to be made, not everything lying around the
    // working tree — so a change that is only in the working tree is not part of the subject.
    let repository = repository_with(
        vec![
            staged("src/main.rs"),
            file_change("notes.md", None, Some(ChangeKind::Modified)),
        ],
        HUNK,
    );
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.commit_message_prompt(project, Path::new(ROOT))
        .expect("prompt");

    assert_eq!(
        repository.reads(),
        3,
        "the status, the recent history, and one diff — the unstaged path was never asked about",
    );
}

#[test]
fn a_change_made_only_of_files_a_tool_wrote_has_nothing_to_describe() {
    // And it is refused from the remembered status alone: nothing is worth spending a subprocess
    // on, and nothing is worth spending an agent on either.
    let repository = repository_with(
        vec![
            staged("pnpm-lock.yaml"),
            staged("Cargo.lock"),
            staged("dist/app.min.js"),
        ],
        HUNK,
    );
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let refusal = git
        .commit_message_prompt(project, Path::new(ROOT))
        .unwrap_err();

    assert!(
        matches!(refusal, GitDraftError::NothingToDescribe),
        "{refusal:?}",
    );
    assert_eq!(
        repository.reads(),
        1,
        "only the status was read; no diff of a file nobody wrote was fetched",
    );
}

#[test]
fn a_resolver_record_beside_a_real_change_is_left_out_of_the_subject() {
    let repository = repository_with(vec![staged("src/main.rs"), staged("Cargo.lock")], HUNK);
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.commit_message_prompt(project, Path::new(ROOT))
        .expect("prompt");

    assert_eq!(
        repository.reads(),
        3,
        "the source file's diff was read and the lock file's was not",
    );
}

#[test]
fn a_change_too_large_to_show_is_described_by_its_paths_instead() {
    let hunk = long_hunk();
    let prompt = prompt_for(repository_with(
        vec![
            staged("a.rs"),
            staged("b.rs"),
            staged("c.rs"),
            staged("d.rs"),
            staged("e.rs"),
        ],
        &hunk,
    ))
    .expect("prompt");

    assert!(
        prompt.contains("modified a.rs\n") && prompt.contains("modified e.rs\n"),
        "every staged path is named, so the whole change is described rather than a tenth of it: {}",
        &prompt[..prompt.len().min(400)],
    );
    assert!(
        !prompt.contains("@@"),
        "no patch survives into a prompt that gave up on showing them",
    );
    assert!(
        prompt.len() <= ONE_SHOT_PROMPT_LIMIT,
        "the composed prompt is within the ceiling it was composed to: {} bytes",
        prompt.len(),
    );
}

#[test]
fn a_change_touching_more_paths_than_are_ever_read_is_summarised_without_reading_any() {
    let many: Vec<FileChange> = (0..80).map(|n| staged(&format!("src/f{n}.rs"))).collect();
    let repository = repository_with(many, HUNK);
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let prompt = git
        .commit_message_prompt(project, Path::new(ROOT))
        .expect("prompt");

    assert!(prompt.contains("modified src/f79.rs\n"), "{prompt}");
    assert_eq!(
        repository.reads(),
        2,
        "the status and the history — a change of this size is never read path by path",
    );
}

#[test]
fn a_summary_of_more_paths_than_fit_says_how_many_were_left_out() {
    // The other half of the ceiling. A change wide enough that even its list of paths does not fit
    // has to stop somewhere, and stopping silently would understate the change.
    let many: Vec<FileChange> = (0..20_000)
        .map(|n| staged(&format!("src/a/rather/long/path/to/file{n}.rs")))
        .collect();

    let prompt = prompt_for(repository_with(many, HUNK)).expect("prompt");

    assert!(
        prompt.contains(" more files\n"),
        "the paths that did not fit are counted rather than dropped in silence: {}",
        &prompt[prompt.len().saturating_sub(200)..],
    );
    assert!(
        prompt.len() <= ONE_SHOT_PROMPT_LIMIT,
        "{} bytes",
        prompt.len(),
    );
}

#[test]
fn a_summary_says_what_happened_to_each_path_and_where_a_move_came_from() {
    let mut moved = file_change("src/new.rs", Some(ChangeKind::Renamed), None);
    moved.original_path = Some("src/old.rs".to_string());
    let many: Vec<FileChange> = (0..60)
        .map(|n| staged(&format!("src/f{n}.rs")))
        .chain([
            moved,
            file_change("gone.rs", Some(ChangeKind::Deleted), None),
            file_change("fresh.rs", Some(ChangeKind::Added), None),
        ])
        .collect();

    let prompt = prompt_for(repository_with(many, "")).expect("prompt");

    assert!(
        prompt.contains("renamed src/new.rs (from src/old.rs)\n"),
        "a move is only legible if both of its names are given: {prompt}",
    );
    assert!(prompt.contains("deleted gone.rs\n"), "{prompt}");
    assert!(prompt.contains("added fresh.rs\n"), "{prompt}");
}

#[test]
fn a_project_that_has_not_been_trusted_composes_nothing() {
    // Drafting runs an agent CLI with the project as its working directory, and an agent CLI reads
    // the project's own configuration — so it runs code the project carries, which is exactly what
    // trust authorises.
    let repository = repository_with(vec![staged("src/main.rs")], HUNK);
    let git = git_over(repository.clone());

    let refusal = git
        .commit_message_prompt(ProjectId::next(), Path::new(ROOT))
        .unwrap_err();

    assert!(matches!(refusal, GitDraftError::Untrusted), "{refusal:?}");
    assert_eq!(
        repository.reads(),
        0,
        "the gate held before the working tree was read at all",
    );
}

#[test]
fn a_folder_under_no_version_control_has_nothing_to_describe() {
    let project = ProjectId::next();
    let git = git_trusting(FakeGitRepository::answering(Vec::new()), project);

    let refusal = git
        .commit_message_prompt(project, Path::new(ROOT))
        .unwrap_err();

    assert!(
        matches!(refusal, GitDraftError::NothingToDescribe),
        "{refusal:?}",
    );
}

#[test]
fn the_branch_the_change_is_on_is_named() {
    // Free — the status already carries it — and often the only place the purpose of a change is
    // written down at all.
    let mut status = git_status("feat/git-ai-commit-message");
    status.changes = vec![staged("src/main.rs")];
    let repository =
        FakeGitRepository::reporting(status).diffing(raw_diff("diff --git a/f b/f\n", &[HUNK]));

    let prompt = prompt_for(repository).expect("prompt");

    assert!(
        prompt.contains("On branch feat/git-ai-commit-message.\n"),
        "{prompt}",
    );
}

#[test]
fn a_detached_head_claims_no_branch() {
    let mut status = git_status("main");
    status.branch.name = None;
    status.changes = vec![staged("src/main.rs")];
    let repository =
        FakeGitRepository::reporting(status).diffing(raw_diff("diff --git a/f b/f\n", &[HUNK]));

    let prompt = prompt_for(repository).expect("prompt");

    assert!(
        !prompt.contains("On branch"),
        "there is no branch to name, so none is claimed: {prompt}",
    );
}

#[test]
fn recent_subjects_are_shown_as_examples_of_how_this_repository_writes_one() {
    // The one thing a diff cannot carry. A commit message's voice is only written down in the
    // repository's own log, so that is where it is read from.
    let prompt = prompt_for(repository_logging(vec![
        commit_entry(
            "aaa",
            "Open a path's diff in a split beside the terminal",
            "Ada",
        ),
        commit_entry(
            "bbb",
            "Show repository state in a persistent right rail",
            "Ada",
        ),
    ]))
    .expect("prompt");

    assert!(
        prompt.contains("- Open a path's diff in a split beside the terminal\n"),
        "{prompt}",
    );
    assert!(
        prompt.contains("- Show repository state in a persistent right rail\n"),
        "{prompt}",
    );
}

#[test]
fn the_examples_are_marked_as_form_and_kept_out_of_the_change() {
    // The failure this guards is a model reaching for an example subject because the diff was hard
    // to read. Two things stop it: the block says outright not to, and it sits before the line that
    // introduces the change, so nothing in it can be read as part of the change.
    let prompt = prompt_for(repository_logging(vec![commit_entry(
        "aaa",
        "Open a path's diff in a split beside the terminal",
        "Ada",
    )]))
    .expect("prompt");

    assert!(prompt.contains("Do not reuse any of them"), "{prompt}");
    let examples = prompt
        .find("- Open a path's diff")
        .expect("the examples block");
    let change = prompt.find("Staged change:\n").expect("the change label");
    assert!(
        examples < change,
        "the examples come before the change, never inside it: {prompt}",
    );
    assert!(
        prompt[examples..change].find("@@").is_none(),
        "no part of the change appears among the examples: {prompt}",
    );
}

#[test]
fn a_commit_nobody_wrote_is_not_an_example_of_how_anybody_writes() {
    let prompt = prompt_for(repository_logging(vec![
        merge_entry("aaa", "Merge pull request #137 from ArtMin96/feat/sortable"),
        commit_entry(
            "bbb",
            "build(deps): bump tauri-action from 0 to 1",
            "dependabot[bot]",
        ),
        commit_entry("ccc", "Revert \"Open a path's diff in a split\"", "Ada"),
        commit_entry(
            "ddd",
            "Stage, discard and commit a change from the rail",
            "Ada",
        ),
    ]))
    .expect("prompt");

    assert!(
        prompt.contains("- Stage, discard and commit a change from the rail\n"),
        "{prompt}",
    );
    for teaches_nothing in [
        "Merge pull request",
        "build(deps)",
        "Revert \"Open a path's diff",
    ] {
        assert!(
            !prompt.contains(teaches_nothing),
            "{teaches_nothing:?} is not anybody's writing and must not be shown as an example: {prompt}",
        );
    }
}

#[test]
fn only_so_many_examples_are_shown_however_long_the_history_is() {
    let long: Vec<CommitEntry> = (0..40)
        .map(|n| commit_entry(&format!("{n:04}"), &format!("Do the {n}th thing"), "Ada"))
        .collect();

    let prompt = prompt_for(repository_logging(long)).expect("prompt");

    let shown = prompt
        .lines()
        .filter(|line| line.starts_with("- Do the "))
        .count();
    assert_eq!(
        shown, 10,
        "ten examples are a demonstration; forty are a budget: {prompt}",
    );
}

#[test]
fn a_repository_with_no_commits_yet_still_asks_the_question() {
    // A first commit, a shallow clone, an orphan branch — all of them reach a prompt with no
    // examples to show, and all of them still have a change to describe.
    let prompt = prompt_for(repository_logging(Vec::new())).expect("prompt");

    assert!(
        !prompt.contains("For style only"),
        "no examples are claimed where there are none: {prompt}",
    );
    assert!(
        prompt.contains("-old\n+new\n"),
        "the change is still the subject: {prompt}",
    );
}

#[test]
fn a_history_made_only_of_commits_nobody_wrote_shows_no_examples() {
    let prompt = prompt_for(repository_logging(vec![
        merge_entry("aaa", "Merge pull request #1"),
        merge_entry("bbb", "Merge pull request #2"),
    ]))
    .expect("prompt");

    assert!(!prompt.contains("For style only"), "{prompt}");
    assert!(prompt.contains("-old\n+new\n"), "{prompt}");
}

#[test]
fn the_context_never_costs_the_change_more_room_than_the_ceiling_allows() {
    // The budget is ordered: what is being asked, then the context it is asked in, then whatever the
    // change can have. Whether that holds depends on where the last patch that fits happens to land,
    // so the patch size is swept rather than picked: one fixture proves the ceiling for one size and
    // says nothing about the size just above it. Every size has to compose a prompt within the
    // ceiling, and has to keep the examples — they are budgeted before the change, not after it.
    let recent: Vec<CommitEntry> = (0..VOICE_EXAMPLE_COUNT)
        .map(|n| {
            commit_entry(
                &format!("{n:04}"),
                &format!("Do the {n}th thing at considerable and deliberate length indeed"),
                "Ada",
            )
        })
        .collect();
    let many: Vec<FileChange> = (0..DESCRIBED_PATHS)
        .map(|n| staged(&format!("src/f{n}.rs")))
        .collect();

    for patch_bytes in (800..1200).step_by(8) {
        let hunk = format!("@@ -1,1 +1,1 @@\n+{}\n", "x".repeat(patch_bytes));
        let prompt = prompt_for(repository_with(many.clone(), &hunk).logging(recent.clone()))
            .expect("prompt");

        assert!(
            prompt.len() <= ONE_SHOT_PROMPT_LIMIT,
            "a {patch_bytes}-byte patch each composed {} bytes, past the {ONE_SHOT_PROMPT_LIMIT}-byte ceiling",
            prompt.len(),
        );
        assert!(
            prompt.contains("For style only"),
            "{patch_bytes}-byte patches"
        );
    }
}
