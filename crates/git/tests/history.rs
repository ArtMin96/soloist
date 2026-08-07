//! The history read against real repositories, so what is asserted is what the installed `git`
//! actually reports for a log rather than a recording of one.

mod fixture;

use std::path::Path;

use fixture::{git, repository_with, write, BRANCH};
use soloist_core::{CommitEntry, GitRepository};
use soloist_git::CliGitRepository;

/// Commits `subject` in `dir` after changing `file`.
fn commit(dir: &Path, file: &str, subject: &str) {
    write(dir, file, &format!("contents for {subject}\n"));
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", subject]);
}

fn history(dir: &Path, skip: usize, limit: usize) -> Vec<CommitEntry> {
    CliGitRepository
        .log(dir, skip, limit)
        .expect("read the history")
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
            CliGitRepository.log(dir.path(), 0, 10),
            Err(soloist_core::GitError::NotARepo)
        ),
        "not a repository is a different answer from a repository with nothing in it",
    );
}

#[test]
fn a_subject_carrying_what_a_person_writes_survives_the_read() {
    // The format separates on NUL precisely because a subject may hold anything else.
    let subject = "Answer the \"lowlight\" surface\tand a=b";
    let dir = repository_with(&["a.txt"]);
    commit(dir.path(), "a.txt", subject);

    assert_eq!(history(dir.path(), 0, 1)[0].subject, subject);
}
