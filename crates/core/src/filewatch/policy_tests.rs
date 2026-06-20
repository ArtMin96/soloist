//! Behavioural tests for the pure matching policy, kept out of the implementation file. No
//! clock, no I/O — just glob matching, relative-to-root resolution, and the default ignores.

use std::path::{Path, PathBuf};

use crate::ids::ProcessId;

use super::*;

const ROOT: &str = "/project";

fn rule(globs: &[&str]) -> WatchRule {
    let set =
        compile(&globs.iter().map(|g| g.to_string()).collect::<Vec<_>>()).expect("globs compile");
    WatchRule::new(ProcessId::from_raw(1), PathBuf::from(ROOT), set)
}

fn under_root(relative: &str) -> PathBuf {
    Path::new(ROOT).join(relative)
}

#[test]
fn a_glob_matches_relative_to_the_project_root() {
    let r = rule(&["src/**/*.rs"]);
    assert!(r.matches(&under_root("src/app/main.rs")));
    assert!(!r.matches(&under_root("docs/readme.md")));
}

#[test]
fn a_star_crosses_path_separators() {
    // Solo's documented behavior: `*` matches across `/`, so `src/*` reaches nested files.
    let r = rule(&["src/*"]);
    assert!(r.matches(&under_root("src/a/b/c.ts")));
}

#[test]
fn default_ignored_directories_never_match_even_a_catch_all_glob() {
    let r = rule(&["**/*.rs"]);
    for ignored in [
        "node_modules/dep.rs",
        ".git/hooks/x.rs",
        "target/debug/build.rs",
        "dist/bundle.rs",
        ".venv/lib/x.rs",
    ] {
        assert!(
            !r.matches(&under_root(ignored)),
            "{ignored} is in an ignored directory and must not restart",
        );
    }
    // A normal source file under the same glob still matches.
    assert!(r.matches(&under_root("src/main.rs")));
}

#[test]
fn a_non_matching_extension_does_not_match() {
    let r = rule(&["src/**/*.rs"]);
    assert!(!r.matches(&under_root("src/styles.css")));
}

#[test]
fn a_change_outside_the_root_never_matches() {
    let r = rule(&["**/*"]);
    assert!(!r.matches(Path::new("/elsewhere/src/main.rs")));
}

#[test]
fn an_empty_glob_list_compiles_to_no_matcher() {
    // Empty (or all-invalid) globs mean the command is not watched at all.
    assert!(compile(&[]).is_none());
}

#[test]
fn a_list_with_a_valid_glob_compiles() {
    assert!(compile(&["*.rs".to_string()]).is_some());
}
