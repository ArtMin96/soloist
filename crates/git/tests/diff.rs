//! Reading one path's diff, and one path's contents, against real repositories built by the
//! same `git` a user would run — so what is asserted is what the installed tool reports for a
//! modification, an addition, a deletion, a rename, an untracked path, and a binary one.

use std::path::Path;

use soloist_core::{DiffTarget, GitRepository, RawFileDiff};
use soloist_git::CliGitRepository;

mod fixture;
use fixture::{git, repository_with, write};

/// The whole patch a reader would be handed: the header with every hunk after it.
fn patch(diff: &RawFileDiff) -> String {
    let hunks: String = diff.hunks.iter().map(|hunk| hunk.text.as_str()).collect();
    format!("{}{}", diff.header, hunks)
}

fn diff_of(dir: &Path, target: DiffTarget, path: &str, original: Option<&str>) -> RawFileDiff {
    CliGitRepository::new()
        .diff(dir, target, path, original)
        .expect("read the diff")
}

#[test]
fn a_modification_reads_the_lines_that_changed_on_the_side_it_is_asked_about() {
    let dir = repository_with(&["src/main.rs"]);
    write(dir.path(), "src/main.rs", "a wholly different line\n");

    let unstaged = diff_of(dir.path(), DiffTarget::Unstaged, "src/main.rs", None);
    assert!(patch(&unstaged).contains("+a wholly different line"));
    assert!(!unstaged.binary);

    let staged = diff_of(dir.path(), DiffTarget::Staged, "src/main.rs", None);
    assert_eq!(
        patch(&staged),
        "",
        "nothing is staged, so the staged side of the change is empty rather than a failure",
    );

    git(dir.path(), &["add", "src/main.rs"]);
    let staged = diff_of(dir.path(), DiffTarget::Staged, "src/main.rs", None);
    assert!(patch(&staged).contains("+a wholly different line"));
}

#[test]
fn a_staged_addition_reads_as_a_new_file() {
    let dir = repository_with(&["src/main.rs"]);
    write(dir.path(), "src/added.rs", "the new file\n");
    git(dir.path(), &["add", "src/added.rs"]);

    let diff = diff_of(dir.path(), DiffTarget::Staged, "src/added.rs", None);

    assert!(patch(&diff).contains("new file mode"));
    assert!(patch(&diff).contains("+the new file"));
}

#[test]
fn a_deletion_reads_as_the_lines_that_are_gone() {
    let dir = repository_with(&["src/main.rs"]);
    std::fs::remove_file(dir.path().join("src/main.rs")).expect("remove");

    let diff = diff_of(dir.path(), DiffTarget::Unstaged, "src/main.rs", None);

    assert!(patch(&diff).contains("deleted file mode"));
    assert!(patch(&diff).contains("-the original contents of src/main.rs"));
}

#[test]
fn a_rename_is_recognised_only_when_both_of_its_names_are_asked_about() {
    let dir = repository_with(&["src/main.rs"]);
    git(dir.path(), &["mv", "src/main.rs", "src/renamed.rs"]);

    let with_both = diff_of(
        dir.path(),
        DiffTarget::Staged,
        "src/renamed.rs",
        Some("src/main.rs"),
    );
    assert!(
        patch(&with_both).contains("rename from src/main.rs"),
        "given both names version control sees the move: {}",
        patch(&with_both),
    );

    let with_one = diff_of(dir.path(), DiffTarget::Staged, "src/renamed.rs", None);
    assert!(
        patch(&with_one).contains("new file mode"),
        "given one it sees an unrelated file appear, which is why the original path is passed",
    );
}

#[test]
fn an_untracked_path_reads_as_the_whole_of_itself() {
    let dir = repository_with(&["src/main.rs"]);
    write(dir.path(), "notes.md", "a line\nanother line\n");

    let diff = diff_of(dir.path(), DiffTarget::Untracked, "notes.md", None);

    assert!(patch(&diff).contains("+a line"));
    assert!(patch(&diff).contains("+another line"));
    assert!(
        !diff.hunks.is_empty(),
        "a path with no earlier version still has a diff, against nothing",
    );
}

#[test]
fn a_binary_path_is_reported_as_binary_rather_than_rendered() {
    let dir = repository_with(&["src/main.rs"]);
    std::fs::write(dir.path().join("icon.bin"), [0u8, 1, 2, 0, 3]).expect("write bytes");
    git(dir.path(), &["add", "icon.bin"]);
    git(dir.path(), &["commit", "-m", "add bytes"]);
    std::fs::write(dir.path().join("icon.bin"), [9u8, 8, 0, 7]).expect("write bytes");

    let diff = diff_of(dir.path(), DiffTarget::Unstaged, "icon.bin", None);

    assert!(diff.binary);
}

#[test]
fn the_head_comparison_sees_a_change_whichever_side_of_the_index_it_is_on() {
    let dir = repository_with(&["src/main.rs"]);
    write(dir.path(), "src/main.rs", "staged line\n");
    git(dir.path(), &["add", "src/main.rs"]);
    write(
        dir.path(),
        "src/main.rs",
        "staged line\nand an unstaged one\n",
    );

    let diff = diff_of(dir.path(), DiffTarget::Head, "src/main.rs", None);

    assert!(patch(&diff).contains("+staged line"));
    assert!(patch(&diff).contains("+and an unstaged one"));
}

#[test]
fn a_folder_outside_any_repository_is_named_as_such_rather_than_failing_obscurely() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), "notes.md", "a line\n");

    let refused = CliGitRepository::new().diff(dir.path(), DiffTarget::Unstaged, "notes.md", None);

    assert!(matches!(refused, Err(soloist_core::GitError::NotARepo)));
}

#[test]
fn a_file_reads_as_the_text_the_working_tree_holds() {
    let dir = repository_with(&["src/main.rs"]);

    let content = CliGitRepository::new()
        .read_file(dir.path(), "src/main.rs")
        .expect("read")
        .expect("a file");

    assert_eq!(
        content.text.as_deref(),
        Some("the original contents of src/main.rs\n"),
    );
    assert!(!content.truncated);
}

#[test]
fn a_file_of_bytes_reads_as_having_no_text_rather_than_as_noise() {
    let dir = repository_with(&["src/main.rs"]);
    std::fs::write(dir.path().join("icon.bin"), [0u8, 1, 2, 0, 3]).expect("write bytes");

    let content = CliGitRepository::new()
        .read_file(dir.path(), "icon.bin")
        .expect("read")
        .expect("a file");

    assert_eq!(content.text, None);
}

#[test]
fn a_path_that_is_no_longer_there_reads_as_nothing_rather_than_failing() {
    let dir = repository_with(&["src/main.rs"]);

    assert_eq!(
        CliGitRepository::new()
            .read_file(dir.path(), "src/gone.rs")
            .expect("no error"),
        None,
    );
}

#[test]
fn a_folder_from_the_listing_reads_as_nothing_rather_than_failing() {
    let dir = repository_with(&["src/main.rs"]);

    assert_eq!(
        CliGitRepository::new()
            .read_file(dir.path(), "src")
            .expect("no error"),
        None,
        "an ignored folder is listed as itself, so being handed one is ordinary",
    );
}
