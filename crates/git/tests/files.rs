//! The file listing against real repositories, each built in a temporary directory by the same
//! `git` a user would run — so what is asserted is what the installed tool actually reports.

use std::path::Path;

use soloist_core::{GitError, GitRepository, ProjectFile};
use soloist_git::CliGitRepository;

mod fixture;
use fixture::{git, write, BRANCH};

fn listing(dir: &Path) -> Vec<ProjectFile> {
    CliGitRepository::new().list_files(dir).expect("list files")
}

fn entry<'a>(files: &'a [ProjectFile], path: &str) -> &'a ProjectFile {
    files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("no entry for {path}: {files:?}"))
}

#[test]
fn a_listing_holds_tracked_and_untracked_paths_and_marks_ignored_ones() {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-b", BRANCH]);
    write(dir.path(), ".gitignore", "build/\n*.log\n");
    write(dir.path(), "src/main.rs", "fn main() {}\n");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    write(dir.path(), "notes.md", "loose\n");
    write(dir.path(), "build/out.o", "binary\n");
    write(dir.path(), "run.log", "noise\n");

    let files = listing(dir.path());

    assert!(!entry(&files, "src/main.rs").ignored, "a tracked file");
    assert!(!entry(&files, "notes.md").ignored, "an untracked file");
    assert!(entry(&files, "run.log").ignored, "an ignored file");
}

#[test]
fn an_ignored_directory_is_listed_as_itself_rather_than_walked() {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-b", BRANCH]);
    write(dir.path(), ".gitignore", "build/\n");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    write(dir.path(), "build/one.o", "binary\n");
    write(dir.path(), "build/deep/two.o", "binary\n");

    let files = listing(dir.path());

    assert!(entry(&files, "build/").ignored);
    assert!(
        !files
            .iter()
            .any(|file| file.path.starts_with("build/o") || file.path.starts_with("build/deep")),
        "an ignored folder's contents must not be listed: {files:?}",
    );
}

#[test]
fn a_folder_that_is_not_a_repository_is_reported_as_such() {
    let dir = tempfile::tempdir().expect("temp dir");

    assert!(matches!(
        CliGitRepository::new().list_files(dir.path()),
        Err(GitError::NotARepo),
    ));
}
