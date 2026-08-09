//! The history read against real repositories, so what is asserted is what the installed `git`
//! actually reports for a log rather than a recording of one.

mod fixture;

use std::path::Path;

use fixture::{git, repository_with, write, BRANCH};
use soloist_core::{CommitEntry, GitRepository, LogRange};
use soloist_git::CliGitRepository;

/// Commits `subject` in `dir` after changing `file`.
fn commit(dir: &Path, file: &str, subject: &str) {
    write(dir, file, &format!("contents for {subject}\n"));
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", subject]);
}

fn history(dir: &Path, skip: usize, limit: usize) -> Vec<CommitEntry> {
    CliGitRepository
        .log(dir, LogRange::CheckedOut, skip, limit)
        .expect("read the history")
}

/// What the checked-out branch holds that `base` does not.
fn proposed(dir: &Path, base: &str) -> Vec<CommitEntry> {
    CliGitRepository
        .log(dir, LogRange::Since { base }, 0, 10)
        .expect("read what the branch proposes")
}

#[test]
fn the_history_reads_back_newest_first_with_what_each_commit_says() {
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", "Do the first thing");
    commit(dir.path(), "a.txt", "Do the second thing");

    let commits = history(dir.path(), 0, 10);

    let subjects: Vec<&str> = commits.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(
        subjects,
        vec!["Do the second thing", "Do the first thing", "start"],
    );
    assert_eq!(commits[0].author, "Fixture");
    assert_eq!(
        commits[0].id.len(),
        40,
        "the full object name, not a prefix"
    );
    assert!(commits[0].authored_at > 0);
    assert!(commits.iter().all(|c| !c.merge));
}

#[test]
fn a_page_is_bounded_and_starts_where_it_was_asked_to() {
    let dir = repository_with(&["a.txt"]);
    for n in 1..=5 {
        commit(dir.path(), "a.txt", &format!("Do thing {n}"));
    }

    let page = history(dir.path(), 2, 2);

    let subjects: Vec<&str> = page.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(subjects, vec!["Do thing 3", "Do thing 2"]);
}

#[test]
fn a_merge_is_reported_as_one_and_the_commits_around_it_are_not() {
    let dir = repository_with(&["a.txt"]);
    git(dir.path(), &["checkout", "-b", "side"]);
    commit(dir.path(), "side.txt", "Do the side thing");
    git(dir.path(), &["checkout", BRANCH]);
    commit(dir.path(), "main.txt", "Do the main thing");
    git(
        dir.path(),
        &["merge", "--no-ff", "-m", "Merge side", "side"],
    );

    let commits = history(dir.path(), 0, 10);

    let merges: Vec<&str> = commits
        .iter()
        .filter(|c| c.merge)
        .map(|c| c.subject.as_str())
        .collect();
    assert_eq!(merges, vec!["Merge side"]);
    assert!(
        commits
            .iter()
            .any(|c| c.subject == "Do the main thing" && !c.merge),
        "an ordinary commit beside a merge is not one",
    );
}

#[test]
fn a_repository_with_no_commits_yet_has_an_empty_history_rather_than_a_failure() {
    // What a fresh `git init` looks like, and what a first commit is made on top of. `git log`
    // exits non-zero here, which is a fact about the repository rather than a failure to read it.
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-b", BRANCH]);

    assert_eq!(history(dir.path(), 0, 10), Vec::new());
}

#[test]
fn a_folder_outside_any_repository_has_no_history_at_all() {
    let dir = tempfile::tempdir().expect("temp dir");

    assert!(
        matches!(
            CliGitRepository.log(dir.path(), LogRange::CheckedOut, 0, 10),
            Err(soloist_core::GitError::NotARepo)
        ),
        "not a repository is a different answer from a repository with nothing in it",
    );
}

#[test]
fn a_commit_message_reads_back_split_into_its_subject_and_the_rest_of_what_it_says() {
    let dir = repository_with(&["a.txt"]);
    write(dir.path(), "a.txt", "changed\n");
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-m",
            "Record the negative result",
            "-m",
            "The engine halted on the second pass.\n\nSo the table is wrong, not the reading.",
        ],
    );

    let commit = &history(dir.path(), 0, 1)[0];

    assert_eq!(commit.subject, "Record the negative result");
    assert_eq!(
        commit.body,
        "The engine halted on the second pass.\n\nSo the table is wrong, not the reading.",
        "a body is the rest of the message over as many lines as it was written in",
    );
}

#[test]
fn a_commit_saying_only_its_subject_has_no_body_rather_than_a_blank_one() {
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", "Do the thing");

    assert_eq!(history(dir.path(), 0, 1)[0].body, "");
}

#[test]
fn a_subject_carrying_what_a_person_writes_survives_the_read() {
    // The format separates on NUL precisely because a subject may hold anything else.
    let subject = "Answer the \"lowlight\" surface\tand a=b";
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", subject);

    assert_eq!(history(dir.path(), 0, 1)[0].subject, subject);
}

#[test]
fn a_range_reads_only_what_the_branch_holds_and_its_base_does_not() {
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", "Do the shared thing");
    git(dir.path(), &["checkout", "-b", "side"]);
    commit(dir.path(), "side.txt", "Do the side thing");

    let proposed = proposed(dir.path(), BRANCH);
    let subjects: Vec<&str> = proposed.iter().map(|c| c.subject.as_str()).collect();

    assert_eq!(
        subjects,
        vec!["Do the side thing"],
        "what a branch proposes is what its base does not already hold, which is a different \
         list from its whole history",
    );
}

#[test]
fn a_branch_holding_nothing_its_base_does_not_reads_as_an_empty_range() {
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", "Do the shared thing");
    git(dir.path(), &["checkout", "-b", "side"]);

    assert_eq!(proposed(dir.path(), BRANCH), Vec::new());
}

#[test]
fn a_base_version_control_cannot_resolve_is_a_failure_rather_than_an_empty_range() {
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", "Do the shared thing");

    assert!(
        matches!(
            CliGitRepository.log(
                dir.path(),
                LogRange::Since {
                    base: "no-such-branch"
                },
                0,
                10
            ),
            Err(soloist_core::GitError::Op { .. }),
        ),
        "reporting no commits for a comparison that never happened would say the branch proposes \
         nothing",
    );
}
