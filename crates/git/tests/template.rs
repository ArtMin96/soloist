//! Resolving `commit.template` in a real repository, built by the same `git` a user would run.
//!
//! Every rule being asserted is version control's own — where a relative path is resolved from,
//! what a tilde expands to, which lines are guidance — so each test states the repository the way
//! a person would configure it and then asks the adapter what a commit there would start from.

use std::path::Path;

use soloist_core::GitRepository;
use soloist_git::CliGitRepository;

mod fixture;
use fixture::{git, repository_with, write};

/// Roomy enough that nothing here is near it; the one test about the ceiling names its own.
const LIMIT: usize = 8 * 1024;

/// What a person would put in a template: a line to keep, and guidance to read and replace.
const TEMPLATE: &str = "\n# Say what changed and why.\nRefs: \n";

/// What is left of [`TEMPLATE`] once version control has removed the guidance, which is exactly
/// what it would have committed had the template been saved unchanged.
const KEPT: &str = "Refs:\n";

fn template_of(dir: &Path) -> Option<String> {
    CliGitRepository::new()
        .commit_template(dir, LIMIT)
        .expect("a template read")
}

#[test]
fn a_repository_that_configures_no_template_starts_a_message_from_nothing() {
    let dir = repository_with(&["notes.md"]);

    assert_eq!(template_of(dir.path()), None);
}

#[test]
fn the_guidance_a_template_carries_is_gone_by_the_time_it_reaches_a_message_box() {
    // The lines version control strips from a message it opened an editor on. Left in, they would
    // be committed verbatim by anybody who typed a subject above them and pressed Commit.
    let dir = repository_with(&["notes.md"]);
    write(dir.path(), "message.txt", TEMPLATE);
    git(dir.path(), &["config", "commit.template", "message.txt"]);

    assert_eq!(template_of(dir.path()), Some(KEPT.to_string()));
}

#[test]
fn a_template_the_repository_carries_is_found_from_a_folder_inside_it() {
    // Where a relative template is resolved from is version control's rule, not a guess: a commit
    // runs from the top of the working tree however deep the folder it was asked for in was.
    let dir = repository_with(&["notes.md"]);
    write(dir.path(), "message.txt", TEMPLATE);
    write(dir.path(), "inner/placeholder", "");
    git(dir.path(), &["config", "commit.template", "message.txt"]);

    assert_eq!(
        template_of(&dir.path().join("inner")),
        Some(KEPT.to_string())
    );
}

#[test]
fn a_template_kept_outside_the_repository_is_read_where_it_actually_lives() {
    // The common case: one personal template, named by an absolute path from a wider
    // configuration, shared across every repository.
    let dir = repository_with(&["notes.md"]);
    let elsewhere = tempfile::tempdir().expect("temp dir");
    let template = elsewhere.path().join("message.txt");
    std::fs::write(&template, TEMPLATE).expect("write the template");
    git(
        dir.path(),
        &["config", "commit.template", &template.to_string_lossy()],
    );

    assert_eq!(template_of(dir.path()), Some(KEPT.to_string()));
}

#[test]
fn a_template_made_only_of_guidance_leaves_a_message_to_be_written_from_nothing() {
    // Version control refuses a commit whose template was saved unedited, because what is left of
    // it is empty. The same file has to leave the box empty rather than filling it with hints.
    let dir = repository_with(&["notes.md"]);
    write(dir.path(), "message.txt", "# Summary\n# Why\n");
    git(dir.path(), &["config", "commit.template", "message.txt"]);

    assert_eq!(template_of(dir.path()), None);
}

#[test]
fn a_template_naming_a_file_that_is_not_there_is_a_message_with_no_template() {
    // Version control refuses the commit outright here. Nothing is being committed yet, so the
    // honest answer is an empty box rather than a repository that cannot be worked in.
    let dir = repository_with(&["notes.md"]);
    git(dir.path(), &["config", "commit.template", "absent.txt"]);

    assert_eq!(template_of(dir.path()), None);
}

#[test]
fn a_template_longer_than_a_message_box_would_hold_is_dropped_whole() {
    // Cut instead, it would be filled in as though it were all of one — the same call the pull
    // request's own skeleton makes.
    let dir = repository_with(&["notes.md"]);
    let ceiling = 64;
    write(dir.path(), "message.txt", &"x".repeat(ceiling + 1));
    git(dir.path(), &["config", "commit.template", "message.txt"]);

    let read = CliGitRepository::new()
        .commit_template(dir.path(), ceiling)
        .expect("a template read");

    assert_eq!(read, None);
}
