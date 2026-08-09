//! What one page of history carries, driving a real [`Git`] over the shared [`FakeGitRepository`].

use std::path::Path;

use crate::git::LogRange;
use crate::ids::ProjectId;
use crate::testing::{commit_entry, git_over, FakeGitRepository};

use super::LOG_PAGE_SIZE;

/// The fake ignores it — a read is addressed by project here, not by path.
const ROOT: &str = "/project";

fn history_of(count: usize) -> FakeGitRepository {
    let commits = (0..count)
        .map(|n| {
            commit_entry(
                &format!("{n:040x}"),
                &format!("Do the {n}th thing"),
                "Somebody",
            )
        })
        .collect();
    FakeGitRepository::answering(Vec::new()).logging(commits)
}

#[test]
fn a_page_starts_where_it_was_asked_to_and_stops_at_the_length_it_was_given() {
    let git = git_over(history_of(10));

    let page = git
        .history(
            ProjectId::next(),
            Path::new(ROOT),
            LogRange::CheckedOut,
            4,
            3,
        )
        .expect("read")
        .expect("a repository");

    let subjects: Vec<&str> = page.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(
        subjects,
        vec!["Do the 4th thing", "Do the 5th thing", "Do the 6th thing"],
    );
}

#[test]
fn no_caller_can_ask_for_more_than_one_page() {
    // The bound is the context's, not the caller's: a surface asking for the whole history gets a
    // page, so nothing here ever holds an unbounded read.
    let git = git_over(history_of(LOG_PAGE_SIZE * 3));

    let page = git
        .history(
            ProjectId::next(),
            Path::new(ROOT),
            LogRange::CheckedOut,
            0,
            usize::MAX,
        )
        .expect("read")
        .expect("a repository");

    assert_eq!(page.len(), LOG_PAGE_SIZE);
}

#[test]
fn a_repository_with_no_commits_yet_has_an_empty_history_rather_than_none() {
    // The two are different answers and a caller acts differently on each: no history at all means
    // there is no repository, where an empty one means there is and nothing has been recorded in it.
    let git = git_over(FakeGitRepository::answering(Vec::new()).logging(Vec::new()));

    let page = git
        .history(
            ProjectId::next(),
            Path::new(ROOT),
            LogRange::CheckedOut,
            0,
            LOG_PAGE_SIZE,
        )
        .expect("read");

    assert_eq!(page, Some(Vec::new()));
}

#[test]
fn a_folder_under_no_version_control_has_no_history() {
    let git = git_over(FakeGitRepository::answering(Vec::new()));

    assert_eq!(
        git.history(
            ProjectId::next(),
            Path::new(ROOT),
            LogRange::CheckedOut,
            0,
            LOG_PAGE_SIZE
        )
        .expect("read"),
        None,
    );
}
