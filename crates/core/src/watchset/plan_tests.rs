//! Behavioural tests for the pure planning policy, kept out of the implementation file. No
//! clock, no I/O — just the fitting arithmetic and the allocation order it depends on.

use std::path::PathBuf;

use crate::filewatch::{Scan, ScannedPath};
use crate::watch::{WatchLimit, WatchPurpose};

use super::*;

const ROOT: &str = "/project";

fn root() -> PathBuf {
    PathBuf::from(ROOT)
}

fn under(relative: &str) -> PathBuf {
    root().join(relative)
}

fn scan(directories: &[&str], truncated: bool) -> Scan {
    Scan {
        paths: directories
            .iter()
            .map(|d| ScannedPath {
                path: under(d),
                directory: true,
            })
            .collect(),
        truncated,
    }
}

fn globs() -> Vec<String> {
    vec!["dist/**/*.json".to_string()]
}

#[test]
fn a_small_tree_fits_and_produces_no_limit() {
    let tree = scan(&["src", "docs"], false);
    let result = plan(&root(), &[], &tree, &[], 10);
    assert!(result.limit.is_empty());
    assert!(result.directories.contains(&under("src")));
    assert!(result.directories.contains(&under("docs")));
}

#[test]
fn an_oversized_tree_degrades_git_status_only_and_still_contains_every_prefix_directory() {
    let prefixes = [scan(&["dist"], false)];
    // root + .git (2) + dist (1) = 3, comfortably under the share of 4; the five-directory tree
    // does not fit alongside it (3 + 5 > 4).
    let tree = scan(&["src", "docs", "build", "huge1", "huge2"], false);
    let result = plan(&root(), &globs(), &tree, &prefixes, 4);

    assert_eq!(
        result.limit.get(&WatchPurpose::GitStatus),
        Some(&WatchLimit::Degraded)
    );
    assert!(!result.limit.contains_key(&WatchPurpose::Restarts));
    assert!(
        result.directories.contains(&under("dist")),
        "the explicit prefix survives even though the speculative tree did not",
    );
    assert!(!result.directories.contains(&under("src")));
}

#[test]
fn oversized_prefixes_degrade_both_purposes() {
    let prefixes = [scan(&["dist", "dist/a", "dist/b", "dist/c"], false)];
    let tree = scan(&["src", "docs", "build"], false);
    let result = plan(&root(), &globs(), &tree, &prefixes, 4);

    assert_eq!(
        result.limit.get(&WatchPurpose::Restarts),
        Some(&WatchLimit::Degraded)
    );
    assert_eq!(
        result.limit.get(&WatchPurpose::GitStatus),
        Some(&WatchLimit::Degraded)
    );
    assert!(!result
        .directories
        .iter()
        .any(|p| p.starts_with(under("dist"))));
}

#[test]
fn a_truncated_scan_degrades_even_when_the_count_would_fit() {
    let tree = scan(&["src"], true);
    let result = plan(&root(), &[], &tree, &[], 100);

    assert_eq!(
        result.limit.get(&WatchPurpose::GitStatus),
        Some(&WatchLimit::Degraded)
    );
    assert!(!result.directories.contains(&under("src")));
}

#[test]
fn the_state_directory_and_its_refs_tree_are_present_even_when_fully_degraded() {
    let prefixes = [scan(&["dist", "a", "b", "c", "d", "e"], false)];
    let tree = scan(&["huge1", "huge2", "huge3"], false);
    let result = plan(&root(), &globs(), &tree, &prefixes, 3);

    assert!(result.directories.contains(&root()));
    assert!(result.directories.contains(&under(".git")));
    assert_eq!(result.trees, vec![under(".git/refs")]);
}

#[test]
fn a_project_with_no_globs_asks_for_no_prefixes_and_reports_no_restarts_entry() {
    let tree = scan(&["src"], false);
    // A caller that (incorrectly) passed a prefix scan anyway must still be ignored: there are
    // no globs to serve one for.
    let result = plan(&root(), &[], &tree, &[scan(&["ignored"], false)], 100);

    assert!(!result.limit.contains_key(&WatchPurpose::Restarts));
    assert!(!result.directories.contains(&under("ignored")));
}

#[test]
fn a_scan_reporting_the_root_itself_is_not_double_counted() {
    // The scanner's own contract is to include the root it was asked to scan; the always-held
    // root must not also consume budget as if it were a distinct find.
    let tree = Scan {
        paths: vec![
            ScannedPath {
                path: root(),
                directory: true,
            },
            ScannedPath {
                path: under("src"),
                directory: true,
            },
        ],
        truncated: false,
    };
    // Budget for exactly root + .git + one more directory (src). If the root were
    // double-counted this would overflow and degrade GitStatus.
    let result = plan(&root(), &[], &tree, &[], 3);

    assert!(result.limit.is_empty());
    assert!(result.directories.contains(&under("src")));
}
