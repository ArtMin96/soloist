//! Where a project's status can change: the directories it is watched in, the watches themselves,
//! and which projects a changed path belongs to.
//!
//! A status has two sources, so a project is watched in two places. The repository state says what
//! is committed and staged: the files directly inside `.git` (`HEAD`, `index`, `packed-refs`,
//! `MERGE_HEAD`, `FETCH_HEAD`) plus the `refs` tree, which is nested. The working tree says what
//! differs from it, and a file edited, added, or deleted there touches nothing under `.git` — so
//! the tree is watched too, as a tree, minus the directories whose churn is never a change worth
//! reading (the shared [`is_ignored`], so the build and dependency trees are named in one place).
//!
//! A project's own root and `.git` are what it watches; a project that is a subdirectory of a
//! repository keeps its status, and its own files still report, but the state above the watched
//! root does not.
//!
//! When a change here is worth re-reading a status is [`super::watch`]'s.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::filewatch::{is_ignored, FileWatcher, WatchHandle};
use crate::ids::ProjectId;
use crate::supervision::run_blocking;
use crate::watch::{WatchError, WatchOutcome};

/// The directory in a project root holding its repository state.
const STATE_DIR: &str = ".git";

/// The subdirectory of [`STATE_DIR`] holding refs. Watched as a tree, because a branch or a
/// remote-tracking ref sits one or more levels inside it.
const REFS_DIR: &str = "refs";

/// The extension git gives the lock files it creates and removes around every write.
const LOCK_EXTENSION: &str = "lock";

/// A watched project: where to read its status from, and the repository state that says when to.
struct Watched {
    root: PathBuf,
    state_dir: PathBuf,
}

impl Watched {
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

    /// The three places this project's status can change: the directory holding its repository
    /// state, the refs tree inside it, and its working tree — each independently watchable, so an
    /// exhausted budget can grant some and refuse the rest.
    fn targets(&self) -> [WatchTarget; 3] {
        [
            WatchTarget {
                path: self.state_dir.clone(),
                recursive: false,
                reported: false,
            },
            WatchTarget {
                path: self.state_dir.join(REFS_DIR),
                recursive: true,
                reported: false,
            },
            WatchTarget {
                path: self.root.clone(),
                recursive: true,
                reported: true,
            },
        ]
    }
}

/// One of the three paths a project is watched at.
#[derive(Clone)]
struct WatchTarget {
    path: PathBuf,
    /// Whether the OS is asked to watch the whole subtree (the working tree, the refs tree) or
    /// only the directory itself (`.git`, whose direct children — `HEAD`, `index`, `packed-refs` —
    /// are what matter).
    recursive: bool,
    /// Whether losing this one is worth telling the user about. Only the working tree's is: a
    /// project that is not a repository has no `.git`, so a state-dir refusal is the ordinary case
    /// rather than a loss, and reporting it would put a notice on every project not under version
    /// control.
    reported: bool,
}

/// What a project has for one watched path: the live watch, or the refusal standing in its place
/// until a later re-sync gets one.
enum Held {
    /// The handle *is* the watch — dropping it releases the OS resources, which is its only job;
    /// nothing here ever reads it back out.
    Watching(#[expect(dead_code)] Box<dyn WatchHandle>),
    Refused(WatchError),
}

/// One open project: where its status is read from, and what it has for each place it is watched.
struct WatchedProject {
    watched: Watched,
    /// Keyed by [`WatchTarget::path`] — the unit of accounting is the path, not the project and
    /// not a role bucket, because refusal is neither total nor evenly split: the budget can grant
    /// `.git` and refuse `.git/refs` as easily as the other way round, and only tracking per path
    /// tells a later re-sync exactly which one to ask for again.
    held: HashMap<PathBuf, Held>,
}

/// The watches held for the open projects, and what each of them covers.
///
/// One record per project, not three parallel maps: an entry's [`WatchedProject::held`] and
/// [`WatchedProject::watched`] describe the same project and can never disagree about which one it
/// is. That is what lets a re-sync ask again for exactly the paths it does not hold — a watch
/// already granted is left alone, and one still refused is asked for again, so a refusal that has
/// since cleared is not permanent.
pub(super) struct Watches {
    watcher: Arc<dyn FileWatcher>,
    changes: mpsc::Sender<PathBuf>,
    projects: HashMap<ProjectId, WatchedProject>,
}

impl Watches {
    /// An empty set, registering its watches through `watcher` and delivering their changes to
    /// `changes`.
    pub(super) fn new(watcher: Arc<dyn FileWatcher>, changes: mpsc::Sender<PathBuf>) -> Self {
        Self {
            watcher,
            changes,
            projects: HashMap::new(),
        }
    }

    /// Watches `project` at `root`, and reports what the OS said about its working-tree watch: the
    /// answer it has just given, or the one standing from an earlier attempt.
    ///
    /// Every path this project is watched at that is not already [`Held::Watching`] — never asked
    /// for, or still refused — is asked for again; a path already granted is left untouched, so an
    /// ordinary re-sync causes no churn.
    pub(super) async fn establish(&mut self, project: ProjectId, root: PathBuf) -> WatchOutcome {
        let watched = Watched::new(root);
        let mut held = self
            .projects
            .remove(&project)
            .map(|entry| entry.held)
            .unwrap_or_default();
        let targets = watched.targets();
        let missing: Vec<WatchTarget> = targets
            .iter()
            .filter(|target| !matches!(held.get(&target.path), Some(Held::Watching(_))))
            .cloned()
            .collect();
        if !missing.is_empty() {
            held.extend(establish_targets(&self.watcher, missing, self.changes.clone()).await);
        }
        let refusal = targets
            .into_iter()
            .find(|target| target.reported)
            .and_then(|target| match held.get(&target.path) {
                Some(Held::Refused(refusal)) => Some(*refusal),
                _ => None,
            });
        self.projects
            .insert(project, WatchedProject { watched, held });
        WatchOutcome { project, refusal }
    }

    /// Drops everything held for a project outside `open` — a project that has been removed —
    /// which releases its OS watches.
    pub(super) fn retain(&mut self, open: &HashSet<ProjectId>) {
        self.projects.retain(|project, _| open.contains(project));
    }

    /// Drops `project`'s watches and forgets its refusals, so the next re-sync establishes
    /// everything again — for a project whose root may have been replaced (a fresh clone over a
    /// deleted checkout is a new inode), where a stale refusal would be as wrong as a stale handle.
    pub(super) fn release(&mut self, project: ProjectId) {
        if let Some(entry) = self.projects.get_mut(&project) {
            entry.held.clear();
        }
    }

    /// Drops every watch and refusal, so the next re-sync establishes them all again.
    pub(super) fn release_all(&mut self) {
        for entry in self.projects.values_mut() {
            entry.held.clear();
        }
    }

    /// Every watched project whose status `path` is part of.
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
            .filter(move |(_, entry)| entry.watched.covers(path))
            .map(|(&project, _)| project)
    }

    /// Where `project`'s status is read from, or `None` for one no longer watched.
    pub(super) fn root_of(&self, project: ProjectId) -> Option<PathBuf> {
        self.projects
            .get(&project)
            .map(|entry| entry.watched.root.clone())
    }
}

/// Registers every target in `targets` independently — one refusal never prevents another target
/// from being asked for — in a single batch off the runtime: the working-tree watch walks the
/// whole tree, so a large repository must never park a runtime worker while the OS enumerates its
/// directories.
///
/// Every refusal is traced, though only the working tree's is reported (see
/// [`WatchTarget::reported`]): a project that is not a repository has no `.git` to watch, so a
/// state-dir refusal is the ordinary case rather than a loss.
async fn establish_targets(
    watcher: &Arc<dyn FileWatcher>,
    targets: Vec<WatchTarget>,
    changes: mpsc::Sender<PathBuf>,
) -> HashMap<PathBuf, Held> {
    let watcher = watcher.clone();
    run_blocking(move || {
        targets
            .into_iter()
            .map(|target| {
                let registered = if target.recursive {
                    watcher.watch(target.path.clone(), changes.clone())
                } else {
                    watcher.watch_dir(target.path.clone(), changes.clone())
                };
                let held = match traced(&target.path, registered) {
                    Ok(handle) => Held::Watching(handle),
                    Err(refusal) => Held::Refused(refusal),
                };
                (target.path, held)
            })
            .collect()
    })
    .await
}

/// Traces a refused watch on `path` and passes the outcome through unchanged, so the watches whose
/// refusal is only worth logging and the one that is also reported say it the same way.
fn traced(
    path: &Path,
    established: Result<Box<dyn WatchHandle>, WatchError>,
) -> Result<Box<dyn WatchHandle>, WatchError> {
    if let Err(refusal) = &established {
        tracing::warn!(
            path = %path.display(),
            %refusal,
            "a repository's changes will not report: the directory could not be watched",
        );
    }
    established
}

/// Whether `path` is one of the lock files git writes around its own writes.
pub(super) fn is_lock(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == LOCK_EXTENSION)
}

#[cfg(test)]
#[path = "watched_tests.rs"]
mod tests;
