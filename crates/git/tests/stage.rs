//! Moving a change across the index of a real repository, built by the same `git` a user would
//! run — so what is asserted is what the installed tool records, not what a recording says it
//! would have.
//!
//! The hunk-level cases are the ones worth the fixtures: a patch built wrongly does not fail, it
//! records something subtly different from what was asked for. Each of them therefore asserts
//! the *contents* of the index rather than only that the operation succeeded, and each ends by
//! putting the change back to prove the reverse is exactly the forward undone.

use std::path::Path;

use soloist_core::{DiffTarget, GitError, GitRepository, HunkRange};
use soloist_git::CliGitRepository;

mod fixture;
use fixture::{git, porcelain_status, repository_with, staged_content, write};

const PATH: &str = "notes.md";

/// A file long enough for two changes to it to be two hunks rather than one — three lines of
/// context on each side of a change is what version control prints by default.
fn many_lines() -> String {
    (1..=30)
        .map(|line| format!("line {line}\n"))
        .collect::<String>()
}

/// The same file with its first and its last line changed, which is two hunks.
fn changed_at_both_ends() -> String {
    many_lines()
        .replace("line 1\n", "LINE ONE\n")
        .replace("line 30\n", "LINE THIRTY\n")
}

/// A repository holding `contents` at [`PATH`], committed.
fn repository(contents: &str) -> tempfile::TempDir {
    let dir = repository_with(&["placeholder"]);
    write(dir.path(), PATH, contents);
    git(dir.path(), &["add", PATH]);
    git(dir.path(), &["commit", "-m", "the file"]);
    dir
}

/// Where each hunk of `path`'s unstaged change falls.
fn unstaged_hunks(dir: &Path, path: &str) -> Vec<HunkRange> {
    CliGitRepository::new()
        .diff(dir, DiffTarget::Unstaged, path, None)
        .expect("read the unstaged diff")
        .hunks
        .iter()
        .map(|hunk| hunk.range)
        .collect()
}

/// Where each hunk of `path`'s staged change falls.
fn staged_hunks(dir: &Path, path: &str) -> Vec<HunkRange> {
    CliGitRepository::new()
        .diff(dir, DiffTarget::Staged, path, None)
        .expect("read the staged diff")
        .hunks
        .iter()
        .map(|hunk| hunk.range)
        .collect()
}

#[test]
fn staging_a_whole_path_records_everything_the_working_tree_holds_for_it() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());

    CliGitRepository::new()
        .stage(dir.path(), PATH, None)
        .expect("stage");

    assert_eq!(porcelain_status(dir.path(), PATH), "M ");
    assert_eq!(staged_content(dir.path(), PATH), changed_at_both_ends());
}

#[test]
fn unstaging_a_whole_path_puts_the_index_back_and_leaves_the_working_tree_alone() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());
    let repository = CliGitRepository::new();
    repository.stage(dir.path(), PATH, None).expect("stage");

    repository.unstage(dir.path(), PATH, None).expect("unstage");

    assert_eq!(porcelain_status(dir.path(), PATH), " M");
    assert_eq!(
        staged_content(dir.path(), PATH),
        many_lines(),
        "the index is back to the commit",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join(PATH)).expect("read"),
        changed_at_both_ends(),
        "and the working tree still holds the change",
    );
}

#[test]
fn staging_the_first_hunk_of_a_file_records_that_change_and_no_other() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());
    let repository = CliGitRepository::new();
    let hunks = unstaged_hunks(dir.path(), PATH);
    assert_eq!(hunks.len(), 2, "the fixture is two hunks");

    repository
        .stage_hunk(dir.path(), PATH, None, hunks[0])
        .expect("stage the first hunk");

    assert_eq!(
        porcelain_status(dir.path(), PATH),
        "MM",
        "part of the change is staged and part is not",
    );
    assert_eq!(
        staged_content(dir.path(), PATH),
        many_lines().replace("line 1\n", "LINE ONE\n"),
        "the index holds the first change and not the last",
    );
}

#[test]
fn staging_the_last_hunk_of_a_file_records_that_change_and_no_other() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());
    let hunks = unstaged_hunks(dir.path(), PATH);

    CliGitRepository::new()
        .stage_hunk(dir.path(), PATH, None, hunks[1])
        .expect("stage the last hunk");

    assert_eq!(
        staged_content(dir.path(), PATH),
        many_lines().replace("line 30\n", "LINE THIRTY\n"),
    );
}

#[test]
fn unstaging_one_hunk_takes_back_exactly_what_staging_it_recorded() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());
    let repository = CliGitRepository::new();
    repository.stage(dir.path(), PATH, None).expect("stage all");
    let staged = staged_hunks(dir.path(), PATH);

    repository
        .unstage_hunk(dir.path(), PATH, None, staged[0])
        .expect("unstage the first hunk");

    assert_eq!(
        staged_content(dir.path(), PATH),
        many_lines().replace("line 30\n", "LINE THIRTY\n"),
        "reversing one hunk leaves the index holding the others",
    );
}

#[test]
fn discarding_one_hunk_restores_those_lines_in_the_working_tree_and_no_others() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());
    let hunks = unstaged_hunks(dir.path(), PATH);

    CliGitRepository::new()
        .discard_hunk(dir.path(), PATH, hunks[0])
        .expect("discard the first hunk");

    assert_eq!(
        std::fs::read_to_string(dir.path().join(PATH)).expect("read"),
        many_lines().replace("line 30\n", "LINE THIRTY\n"),
        "the first change is gone from the working tree and the last one is untouched",
    );
}

#[test]
fn a_hunk_the_diff_no_longer_holds_changes_nothing_at_all() {
    let dir = repository(&many_lines());
    write(dir.path(), PATH, &changed_at_both_ends());

    let refusal = CliGitRepository::new()
        .stage_hunk(
            dir.path(),
            PATH,
            None,
            HunkRange {
                old_start: 900,
                old_lines: 3,
                new_start: 900,
                new_lines: 3,
            },
        )
        .unwrap_err();

    assert_eq!(refusal, GitError::HunkGone);
    assert_eq!(
        porcelain_status(dir.path(), PATH),
        " M",
        "nothing was staged",
    );
}

#[test]
fn a_hunk_of_a_file_with_windows_line_endings_keeps_them_exactly() {
    let original = "alpha\r\nbravo\r\ncharlie\r\n";
    let changed = "alpha\r\nBRAVO\r\ncharlie\r\n";
    let dir = repository(original);
    write(dir.path(), PATH, changed);
    let hunks = unstaged_hunks(dir.path(), PATH);

    CliGitRepository::new()
        .stage_hunk(dir.path(), PATH, None, hunks[0])
        .expect("stage the hunk");

    assert_eq!(
        staged_content(dir.path(), PATH),
        changed,
        "a carriage return is part of the line, so a rebuilt context line would not match",
    );
}

#[test]
fn a_hunk_at_the_end_of_a_file_that_has_no_final_newline_keeps_it_that_way() {
    let original = "alpha\nbravo\ncharlie";
    let changed = "alpha\nbravo\nCHARLIE";
    let dir = repository(original);
    write(dir.path(), PATH, changed);
    let hunks = unstaged_hunks(dir.path(), PATH);

    CliGitRepository::new()
        .stage_hunk(dir.path(), PATH, None, hunks[0])
        .expect("stage the hunk");

    assert_eq!(
        staged_content(dir.path(), PATH),
        changed,
        "the marker version control prints for a missing final newline has to survive the patch",
    );
}

#[test]
fn a_renamed_path_is_staged_as_one_move_rather_than_a_deletion_and_an_arrival() {
    let dir = repository(&many_lines());
    std::fs::rename(dir.path().join(PATH), dir.path().join("renamed.md")).expect("rename");

    CliGitRepository::new()
        .stage(dir.path(), "renamed.md", Some(PATH))
        .expect("stage the rename");

    assert!(
        fixture::git_output(dir.path(), &["status", "--porcelain"])
            .contains("R  notes.md -> renamed.md"),
        "given both names version control records a move; given one it records half of it",
    );
}

#[test]
fn unstaging_a_renamed_path_puts_both_of_its_names_back() {
    let dir = repository(&many_lines());
    std::fs::rename(dir.path().join(PATH), dir.path().join("renamed.md")).expect("rename");
    let repository = CliGitRepository::new();
    repository
        .stage(dir.path(), "renamed.md", Some(PATH))
        .expect("stage the rename");

    repository
        .unstage(dir.path(), "renamed.md", Some(PATH))
        .expect("unstage the rename");

    let status = fixture::git_output(dir.path(), &["status", "--porcelain"]);
    assert!(status.contains(" D notes.md"), "{status}");
    assert!(status.contains("?? renamed.md"), "{status}");
}
