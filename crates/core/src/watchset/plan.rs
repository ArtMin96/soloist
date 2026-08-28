//! The pure registration policy: given what a scan found and how much budget a project has,
//! decide exactly which directories to register and what — if anything — had to be dropped.
//!
//! No clock, no I/O: this is handed [`Scan`]s a caller already ran off the runtime, so it is
//! exhaustively unit-testable on its own, the same shape as [`crate::filewatch::policy`].

use std::collections::{HashMap, HashSet};
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
/// distinct directory the globs ask to have watched (`crate::filewatch::literal_prefix`), each
/// scanned with the repository's own ignore rules disabled — a gitignored directory stays
/// watched whether a glob names it or merely reaches it. `tree` is the repository-ignore-
/// honouring scan of the whole root.
///
/// Each purpose is degraded only by what it asked for and did not get: a project whose tree does
/// not fit degrades [`WatchPurpose::GitStatus`] alone, while one that could not have every
/// prefix degrades [`WatchPurpose::Restarts`] too, and leaves less room for the tree besides.
/// `prefixes` is fitted in the order given — [`Fitting`] states the rule — so the caller passes
/// them in the order the project can least afford to lose them.
pub(crate) fn plan(
    root: &Path,
    globs: &[String],
    tree: &Scan,
    prefixes: &[Scan],
    share: usize,
) -> ProjectPlan {
    let state_dir = root.join(STATE_DIR);
    let trees = vec![state_dir.join(REFS_DIR)];
    let mut fitting = Fitting::new(share, vec![root.to_path_buf(), state_dir], trees.len());
    let mut limit = HashMap::new();

    if !globs.is_empty() {
        let mut every_prefix = true;
        for prefix in prefixes {
            every_prefix &= fitting.fit(prefix);
        }
        if !every_prefix {
            limit.insert(WatchPurpose::Restarts, WatchLimit::Degraded);
        }
    }
    if !fitting.fit(tree) {
        limit.insert(WatchPurpose::GitStatus, WatchLimit::Degraded);
    }

    ProjectPlan {
        trees,
        directories: fitting.directories,
        limit,
    }
}

/// The registration set being assembled: what it holds, and how much of the share that has
/// spent.
///
/// The fitting rule, stated once. Scans are offered one at a time, in the order the project can
/// least afford to lose them: the directories each glob names, then the whole-root scan a
/// prefix-less glob asks for, then the speculative tree. A scan is taken whole or not at all, so
/// one too large for what is left costs only itself and the scans after it. A directory an
/// earlier scan already took is neither registered nor charged again. A truncated scan is dropped
/// individually, however many paths it did report, because half a walk is a watch set that lies
/// about its coverage.
struct Fitting {
    share: usize,
    spent: usize,
    taken: HashSet<PathBuf>,
    directories: Vec<PathBuf>,
}

impl Fitting {
    /// A fitting holding the paths a project is watched at regardless — its root and its
    /// repository state directory — with `trees` subtree registrations already charged against
    /// the share alongside them.
    fn new(share: usize, always: Vec<PathBuf>, trees: usize) -> Self {
        Self {
            share,
            spent: always.len() + trees,
            taken: always.iter().cloned().collect(),
            directories: always,
        }
    }

    /// Takes every directory `scan` found that is not held already, if all of it fits in what is
    /// left of the share, and reports whether it was taken.
    fn fit(&mut self, scan: &Scan) -> bool {
        if scan.truncated {
            return false;
        }
        let mut found: Vec<PathBuf> = Vec::new();
        let mut adding: HashSet<&Path> = HashSet::new();
        for path in scan
            .paths
            .iter()
            .filter(|scanned| scanned.directory)
            .map(|scanned| scanned.path.as_path())
        {
            if !self.taken.contains(path) && adding.insert(path) {
                found.push(path.to_path_buf());
            }
        }
        if self.spent + found.len() > self.share {
            return false;
        }
        self.spent += found.len();
        self.taken.extend(found.iter().cloned());
        self.directories.extend(found);
        true
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
