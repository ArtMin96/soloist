//! Which projects a changed path belongs to, and where each project's status is read from.
//!
//! A status has two sources, so a project is watched in two places. The repository state says what
//! is committed and staged: the files directly inside `.git` (`HEAD`, `index`, `packed-refs`,
//! `MERGE_HEAD`, `FETCH_HEAD`) plus the `refs` tree, which is nested. The working tree says what
//! differs from it, and a file edited, added, or deleted there touches nothing under `.git` — so
//! the tree is watched too, minus the directories whose churn is never a change worth reading (the
//! shared [`is_ignored`], so the build and dependency trees are named in one place).
//!
//! A project's own root and `.git` are what it watches; a project that is a subdirectory of a
//! repository keeps its status, and its own files still report, but the state above the watched
//! root does not.
//!
//! This module is pure routing: registering, budgeting, and re-establishing every project's OS
//! watches is [`crate::watchset::ProjectWatchSet`]'s job entirely, over the single shared
//! [`crate::filewatch::WatchSession`]. What survives here is where each open project's status is
//! scoped to, so a changed path reported on the watch set's fan-out can be matched back to the
//! projects it belongs to. When a change here is worth re-reading a status is [`super::watch`]'s.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::filewatch::is_ignored;
use crate::ids::ProjectId;
use crate::vcs::STATE_DIR;

/// The extension git gives the lock files it creates and removes around every write.
const LOCK_EXTENSION: &str = "lock";

/// Where one project's status is read from, and the repository state that says when to read it.
struct Route {
    root: PathBuf,
    state_dir: PathBuf,
}

impl Route {
    fn new(root: PathBuf) -> Self {
        let state_dir = root.join(STATE_DIR);
        Self { root, state_dir }
    }

    /// Whether a change to `path` is part of this project's status — through its repository state
    /// or its working tree.
    ///
    /// The working tree is matched by the project root minus the ignored directories, and `.git` is
    /// one of them, so a repository-state path can only ever match through `state_dir`. That is
    /// what keeps the tree watch (which spans `.git` too, being recursive over the root) from
    /// making one `.git` write look like two different changes.
    fn covers(&self, path: &Path) -> bool {
        path.starts_with(&self.state_dir)
            || path
                .strip_prefix(&self.root)
                .is_ok_and(|relative| !is_ignored(relative))
    }
}

/// Every open project's routing: which paths change its status, and where its status is read
/// from.
pub(super) struct Routes {
    projects: HashMap<ProjectId, Route>,
}

impl Routes {
    /// Nothing routed yet.
    pub(super) fn new() -> Self {
        Self {
            projects: HashMap::new(),
        }
    }

    /// Routes `project`'s status to `root`, replacing whatever it was routed to before — for a
    /// project whose root may have been replaced (a fresh clone over a deleted checkout is a new
    /// inode), where a stale route would be as wrong as a stale watch.
    pub(super) fn set(&mut self, project: ProjectId, root: PathBuf) {
        self.projects.insert(project, Route::new(root));
    }

    /// Drops everything routed for a project outside `open` — a project that has been removed.
    pub(super) fn retain(&mut self, open: &HashSet<ProjectId>) {
        self.projects.retain(|project, _| open.contains(project));
    }

    /// Every currently routed project — for a lagged change stream, where the specific path that
    /// was lost is unknown and every project's status has to be treated as possibly stale.
    pub(super) fn routed(&self) -> impl Iterator<Item = ProjectId> + '_ {
        self.projects.keys().copied()
    }

    /// Every routed project whose status `path` is part of.
    ///
    /// More than one can match, because projects nest: a repository opened inside another project's
    /// tree shares its files, so a file changed there changes what both statuses say. Each is armed,
    /// since each is a status of its own with a rail of its own — and a project whose status turns
    /// out not to have changed announces nothing, so the extra read costs a subprocess and no more.
    pub(super) fn projects_of<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Iterator<Item = ProjectId> + 'a {
        self.projects
            .iter()
            .filter(move |(_, route)| route.covers(path))
            .map(|(&project, _)| project)
    }

    /// Where `project`'s status is read from, or `None` for one no longer routed.
    pub(super) fn root_of(&self, project: ProjectId) -> Option<PathBuf> {
        self.projects.get(&project).map(|route| route.root.clone())
    }
}

/// Whether `path` is one of the lock files git writes around its own writes.
pub(super) fn is_lock(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == LOCK_EXTENSION)
}
