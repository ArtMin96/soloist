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

use std::collections::hash_map::Entry;
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
}

/// What establishing one project's watches produced.
struct Established {
    handles: Vec<Box<dyn WatchHandle>>,
    /// The working tree's refusal, if it was turned down.
    refusal: Option<WatchError>,
}

/// The watches held for the open projects, and what each of them covers.
///
/// Three lifetimes, kept apart on purpose. A handle *is* a watch — dropping it releases the OS
/// resources — so an entry in `handles` lives exactly as long as its project stays watched. A
/// refusal is what the OS said when those watches were established, kept because a re-sync reports
/// every open project's standing answer and does not re-establish the watches it already holds.
/// And `watched` is rebuilt from the registry on every re-sync, so a project matched and read at
/// the path it has now.
pub(super) struct Watches {
    watcher: Arc<dyn FileWatcher>,
    changes: mpsc::Sender<PathBuf>,
    handles: HashMap<ProjectId, Vec<Box<dyn WatchHandle>>>,
    refusals: HashMap<ProjectId, WatchError>,
    watched: HashMap<ProjectId, Watched>,
}

impl Watches {
    /// An empty set, registering its watches through `watcher` and delivering their changes to
    /// `changes`.
    pub(super) fn new(watcher: Arc<dyn FileWatcher>, changes: mpsc::Sender<PathBuf>) -> Self {
        Self {
            watcher,
            changes,
            handles: HashMap::new(),
            refusals: HashMap::new(),
            watched: HashMap::new(),
        }
    }

    /// Watches `project` at `root`, and reports what the OS said: the answer it has just given, or
    /// the one it gave when the watches this project already holds were established.
    ///
    /// A project already watched keeps its watches, so a re-sync causes no churn.
    pub(super) async fn establish(&mut self, project: ProjectId, root: PathBuf) -> WatchOutcome {
        let watched = Watched::new(root);
        if let Entry::Vacant(slot) = self.handles.entry(project) {
            let established = establish(&self.watcher, &watched, self.changes.clone()).await;
            slot.insert(established.handles);
            match established.refusal {
                Some(refusal) => self.refusals.insert(project, refusal),
                None => self.refusals.remove(&project),
            };
        }
        self.watched.insert(project, watched);
        WatchOutcome {
            project,
            refusal: self.refusals.get(&project).copied(),
        }
    }

    /// Drops everything held for a project outside `open` — a project that has been removed — which
    /// releases its OS watches.
    pub(super) fn retain(&mut self, open: &HashSet<ProjectId>) {
        self.handles.retain(|project, _| open.contains(project));
        self.refusals.retain(|project, _| open.contains(project));
        self.watched.retain(|project, _| open.contains(project));
    }

    /// Drops `project`'s watches so the next re-sync establishes them again.
    pub(super) fn release(&mut self, project: ProjectId) {
        self.handles.remove(&project);
    }

    /// Drops every watch, so the next re-sync establishes them all again.
    pub(super) fn release_all(&mut self) {
        self.handles.clear();
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
        self.watched
            .iter()
            .filter(move |(_, watched)| watched.covers(path))
            .map(|(&project, _)| project)
    }

    /// Where `project`'s status is read from, or `None` for one no longer watched.
    pub(super) fn root_of(&self, project: ProjectId) -> Option<PathBuf> {
        self.watched
            .get(&project)
            .map(|watched| watched.root.clone())
    }
}

/// The watches one project's status needs: its repository state, the refs tree inside it, and its
/// working tree.
///
/// Registering them reads the filesystem — the working-tree watch walks the whole tree — so all
/// three go to the blocking pool together: a large repository must never park a runtime worker
/// while the OS enumerates its directories.
///
/// Each is independent, so what can be watched is. A refused working-tree watch still leaves what
/// version control itself writes reporting, which is why the repository-state watches are separate
/// from the tree watch that spans them: the tree watch is the one an exhausted watch budget refuses
/// first, and losing it must not take committing and staging down with it.
///
/// Only the working tree's refusal is reported, though every refusal is traced. A project that is
/// not a repository has no `.git` to watch, so a state-dir refusal is the ordinary case rather than
/// a loss — reporting it would put a notice on every project not under version control. The
/// working-tree watch is the one every project has, and the one whose silence is indistinguishable
/// from a tree nobody edits.
async fn establish(
    watcher: &Arc<dyn FileWatcher>,
    watched: &Watched,
    changes: mpsc::Sender<PathBuf>,
) -> Established {
    let watcher = watcher.clone();
    let state_dir = watched.state_dir.clone();
    let refs_dir = state_dir.join(REFS_DIR);
    let root = watched.root.clone();
    run_blocking(move || {
        let mut handles: Vec<Box<dyn WatchHandle>> = [
            (
                state_dir.clone(),
                watcher.watch_dir(state_dir, changes.clone()),
            ),
            (refs_dir.clone(), watcher.watch(refs_dir, changes.clone())),
        ]
        .into_iter()
        .filter_map(|(path, established)| traced(&path, established).ok())
        .collect();
        let refusal = match traced(&root, watcher.watch(root.clone(), changes)) {
            Ok(handle) => {
                handles.push(handle);
                None
            }
            Err(refusal) => Some(refusal),
        };
        Established { handles, refusal }
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
