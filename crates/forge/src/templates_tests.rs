//! Tests for finding the description skeletons a repository carries, against real files on disk —
//! the convention is about where a file is and what it is called, so nothing here is mocked.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::{detect, SIZE_LIMIT};

/// Writes `body` at `path` under `root`, creating whatever folders it needs.
fn write(root: &Path, path: &str, body: &str) {
    let file = root.join(path);
    fs::create_dir_all(file.parent().expect("a parent")).expect("folders");
    fs::write(file, body).expect("write");
}

#[test]
fn a_repository_that_states_no_convention_offers_no_skeleton() {
    let dir = TempDir::new().expect("temp dir");
    write(dir.path(), "README.md", "# Not a template");

    assert_eq!(detect(dir.path()), Vec::new());
}

#[test]
fn the_single_skeleton_a_repository_carries_is_offered_with_its_body() {
    let dir = TempDir::new().expect("temp dir");
    write(
        dir.path(),
        ".github/pull_request_template.md",
        "## What changed\n",
    );

    let found = detect(dir.path());

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].body, "## What changed\n");
}

#[test]
fn the_name_is_matched_however_the_repository_capitalised_it() {
    let dir = TempDir::new().expect("temp dir");
    write(dir.path(), "PULL_REQUEST_TEMPLATE.md", "## Shouting\n");

    assert_eq!(
        detect(dir.path())
            .into_iter()
            .map(|template| template.body)
            .collect::<Vec<_>>(),
        vec!["## Shouting\n"],
    );
}

#[test]
fn a_skeleton_with_no_extension_at_all_is_still_one() {
    let dir = TempDir::new().expect("temp dir");
    write(dir.path(), "docs/pull_request_template", "## Bare\n");

    assert_eq!(detect(dir.path()).len(), 1);
}

#[test]
fn a_repository_offering_several_offers_all_of_them_under_their_own_names() {
    let dir = TempDir::new().expect("temp dir");
    write(
        dir.path(),
        ".github/PULL_REQUEST_TEMPLATE/feature.md",
        "## The feature\n",
    );
    write(
        dir.path(),
        ".github/PULL_REQUEST_TEMPLATE/bugfix.md",
        "## The bug\n",
    );

    assert_eq!(
        detect(dir.path())
            .into_iter()
            .map(|template| template.name)
            .collect::<Vec<_>>(),
        vec!["bugfix", "feature"],
        "sorted by name, so the order a repository's own templates appear in is the same twice \
         running rather than the filesystem's",
    );
}

#[test]
fn the_first_place_the_convention_is_stated_is_the_one_that_answers() {
    let dir = TempDir::new().expect("temp dir");
    write(
        dir.path(),
        ".github/pull_request_template.md",
        "## Hidden\n",
    );
    write(dir.path(), "pull_request_template.md", "## Root\n");
    write(dir.path(), "docs/pull_request_template.md", "## Docs\n");

    assert_eq!(
        detect(dir.path())
            .into_iter()
            .map(|template| template.body)
            .collect::<Vec<_>>(),
        vec!["## Hidden\n"],
        "a repository does not offer two sets of templates, so one place answers",
    );
}

#[test]
fn a_skeleton_past_the_ceiling_is_not_offered_rather_than_read_whole() {
    let dir = TempDir::new().expect("temp dir");
    let enormous = "x".repeat(SIZE_LIMIT as usize + 1);
    write(dir.path(), ".github/pull_request_template.md", &enormous);

    assert_eq!(
        detect(dir.path()),
        Vec::new(),
        "past the ceiling it has stopped being a shape to fill in",
    );
}

#[test]
fn a_directory_of_skeletons_only_offers_the_files_that_could_be_one() {
    let dir = TempDir::new().expect("temp dir");
    write(
        dir.path(),
        ".github/PULL_REQUEST_TEMPLATE/feature.md",
        "## The feature\n",
    );
    write(
        dir.path(),
        ".github/PULL_REQUEST_TEMPLATE/icon.png",
        "not text anybody fills in",
    );

    assert_eq!(
        detect(dir.path())
            .into_iter()
            .map(|template| template.name)
            .collect::<Vec<_>>(),
        vec!["feature"],
    );
}
