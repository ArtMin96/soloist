//! The pure registration policy: given what a scan found and how much budget a project has,
//! decide exactly which directories to register and what — if anything — had to be dropped.
//!
//! No clock, no I/O: this is handed [`Scan`]s a caller already ran off the runtime, so it is
//! exhaustively unit-testable on its own, the same shape as [`crate::filewatch::policy`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::filewatch::Scan;
use crate::vcs::{REFS_DIR, STATE_DIR};
use crate::watch::{WatchLimit, WatchPurpose};

/// What one project's watches should be, and what had to be dropped to make them fit.
pub(crate) struct ProjectPlan {
    /// Registered as a bounded subtree (`WatchSession::watch_tree`) — the repository's refs
    /// tree only; everything else is registered non-recursively.
    pub(crate) trees: Vec<PathBuf>,
    /// Registered one directory at a time (`WatchSession::watch_dir`): the project root, its
    /// repository state directory, and whatever the prefix and whole-tree scans found.
    pub(crate) directories: Vec<PathBuf>,
    /// Which purposes had to give up the speculative whole-tree scan, or the glob-prefix scan,
    /// for want of budget. Never carries a [`WatchLimit::Refused`] — that is a fact about
    /// whether a *specific* registration succeeded, which this pure function has no way to
    /// know; it only ever reports [`WatchLimit::Degraded`].
    pub(crate) limit: HashMap<WatchPurpose, WatchLimit>,
}

/// Plans one project's watches from what it always needs, what its `restart_when_changed`
/// globs asked for explicitly, and what the whole tree looks like — in that order, because
/// explicit user intent is budgeted before the speculative whole-tree scan.
///
/// `globs` decides whether the project has anything to ask [`WatchPurpose::Restarts`] for at
/// all: with none, `prefixes` is never read and no `Restarts` entry is ever produced, matching
/// what a caller with no watch-eligible commands would pass. `prefixes` holds one [`Scan`] per
/// distinct glob literal prefix (`crate::filewatch::literal_prefix`), scanned with the
/// repository's own ignore rules disabled — a gitignored prefix a glob names explicitly is
/// still watched. `tree` is the repository-ignore-honouring scan of the whole root.
///
/// Fitting is sequential and independent per purpose: the prefixes are tried against `share`
/// first, and only what they leave is offered to the tree scan — so a project whose tree does
/// not fit degrades only [`WatchPurpose::GitStatus`], while one whose *prefixes* do not fit
/// degrades [`WatchPurpose::Restarts`] too, and leaves less room for the tree besides. A
/// truncated scan is dropped outright regardless of how many paths it did report: half a tree
/// is a watch set that lies about coverage.
pub(crate) fn plan(
    root: &Path,
    globs: &[String],
    tree: &Scan,
    prefixes: &[Scan],
    share: usize,
) -> ProjectPlan {
    let state_dir = root.join(STATE_DIR);
    let refs_dir = state_dir.join(REFS_DIR);
    // The always-watched paths a scan may report on its own (the root, or a prefix that
    // happens to coincide with it), so they are never counted, or registered, twice.
    let held: [PathBuf; 2] = [root.to_path_buf(), state_dir.clone()];
    let mut directories = held.to_vec();
    let trees = vec![refs_dir];
    let mut limit = HashMap::new();
    let mut spent = directories.len();

    if !globs.is_empty() {
        let found = directories_of(prefixes, &held);
        let truncated = prefixes.iter().any(|scan| scan.truncated);
        if !truncated && spent + found.len() <= share {
            spent += found.len();
            directories.extend(found);
        } else {
            limit.insert(WatchPurpose::Restarts, WatchLimit::Degraded);
        }
    }

    let found = directories_of(std::slice::from_ref(tree), &held);
    if !tree.truncated && spent + found.len() <= share {
        directories.extend(found);
    } else {
        limit.insert(WatchPurpose::GitStatus, WatchLimit::Degraded);
    }

    ProjectPlan {
        trees,
        directories,
        limit,
    }
}

/// Every directory a set of scans found, minus the always-watched paths a scan may report on
/// its own (the root, or a prefix that happens to coincide with it).
fn directories_of(scans: &[Scan], exclude: &[PathBuf; 2]) -> Vec<PathBuf> {
    scans
        .iter()
        .flat_map(|scan| scan.paths.iter())
        .filter(|found| found.directory && !exclude.contains(&found.path))
        .map(|found| found.path.clone())
        .collect()
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
