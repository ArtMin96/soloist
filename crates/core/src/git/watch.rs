//! The live-status trigger: a [`Clock`]-driven reactor that turns changes to a repository's
//! own state files into debounced status re-reads.
//!
//! The reactor watches each open project's repository state — the files directly inside `.git`
//! (`HEAD`, `index`, `packed-refs`, `MERGE_HEAD`, `FETCH_HEAD`) plus the `refs` tree, which is
//! nested — coalesces the several writes one `git` invocation makes with the shared
//! [`crate::debounce::Debouncer`], and re-reads that project's status through [`Git`]. It
//! announces [`DomainEvent::GitStatusChanged`] only when the re-read differs from what was
//! already known, so churn that leaves the working tree looking the same wakes no surface.
//!
//! Two rules keep it from fighting the tool it observes. The lock files git creates and removes
//! around every write are ignored: the write they guard reports separately when it lands, and
//! reacting to one only races a write still in progress. And a read that fails is retried
//! **once**, after another quiet window — a status run can still lose that race — so a
//! momentary failure corrects itself without the core polling or ever looping.
//!
//! It watches a project's own `.git`; a project that is a subdirectory of a repository keeps its
//! status but is not watched, because the state that would change lives above the watched root.
//!
//! Like the other filesystem reactors it re-syncs on [`DomainEvent::ProjectOpened`] and
//! [`DomainEvent::ProjectRemoved`], holds the context weakly so it never keeps the app alive,
//! and ends when the bus closes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{FileWatcher, WatchHandle};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::projects::Projects;
use crate::supervision::run_blocking;

use super::status::Git;

/// The quiet window a burst of repository-state changes is coalesced into before one status
/// read. Long enough to absorb the several files a single `git` invocation writes, short enough
/// that a commit made in a terminal beside the rail shows up as it happens.
const QUIET: Duration = Duration::from_millis(100);

/// How many pending changed paths the watch channel buffers before the adapter's sends start
/// dropping. Bounded (no unbounded channel); a dropped path is harmless — the burst it belongs
/// to has already armed the debounce, and the next change re-arms it.
const CHANGE_BUFFER: usize = 256;

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

/// Turns repository-state changes into debounced status re-reads. Built once by the composition
/// root (via [`crate::facade::Facade::git_status_watch_loop`]) and spawned on the runtime.
pub struct GitStatusWatchReactor {
    clock: Arc<dyn Clock>,
    watcher: Arc<dyn FileWatcher>,
    events: broadcast::Receiver<DomainEvent>,
    bus: EventBus,
    git: Weak<Git>,
    projects: Arc<Projects>,
}

impl GitStatusWatchReactor {
    /// Builds a reactor over the file watcher and clock, watching the git context weakly (so it
    /// never keeps the app alive) and subscribing to the bus for project lifecycle and the
    /// shutdown signal.
    pub fn new(
        clock: Arc<dyn Clock>,
        watcher: Arc<dyn FileWatcher>,
        bus: &EventBus,
        git: Weak<Git>,
        projects: Arc<Projects>,
    ) -> Self {
        Self {
            clock,
            watcher,
            events: bus.subscribe(),
            bus: bus.clone(),
            git,
            projects,
        }
    }

    /// Runs the reactor until the bus closes (app shutdown) or the git context is dropped.
    pub async fn run(mut self) {
        let (changes_tx, mut changes_rx) = mpsc::channel(CHANGE_BUFFER);
        // The watch state, held for the reactor's lifetime. `watches` keeps each project's OS
        // watches alive: dropping a handle stops its watch (the bounded-resource contract), so
        // a handle lives exactly as long as its project is open. `watched` is what a change
        // event is matched against and where a re-read runs.
        let mut watches: HashMap<ProjectId, Vec<Box<dyn WatchHandle>>> = HashMap::new();
        let mut watched: HashMap<ProjectId, Watched> = HashMap::new();
        self.resync(&changes_tx, &mut watches, &mut watched);

        let mut debouncers: HashMap<ProjectId, Debouncer> = HashMap::new();
        // The projects whose pending read is already a retry, so a repository that keeps failing
        // is re-read once and then left alone rather than re-armed for ever.
        let mut retried: HashSet<ProjectId> = HashSet::new();
        loop {
            let next_due = debouncers.values().filter_map(Debouncer::due_at).min();
            tokio::select! {
                // The event bus drives two things: a closed bus means the facade dropped, so
                // stop; a project opening or being removed (or a lag that may have hidden
                // either) means the watched set changed, so re-sync. Repository changes
                // themselves arrive on `changes_rx`, not here.
                result = self.events.recv() => {
                    match result {
                        Err(RecvError::Closed) => break,
                        // Opening a project drops its existing watch first: the same path can
                        // have been replaced since (a fresh clone over a deleted checkout is a
                        // new inode), which silently invalidates the OS watch.
                        Ok(DomainEvent::ProjectOpened { id }) => {
                            watches.remove(&id);
                            self.resync(&changes_tx, &mut watches, &mut watched);
                        }
                        Ok(DomainEvent::ProjectRemoved { id }) => {
                            if let Some(git) = self.git.upgrade() {
                                git.forget(id);
                            }
                            debouncers.remove(&id);
                            retried.remove(&id);
                            self.resync(&changes_tx, &mut watches, &mut watched);
                        }
                        // A lag may have hidden an open whose directory was replaced, so rebuild
                        // every watch rather than trust the ones we hold.
                        Err(RecvError::Lagged(_)) => {
                            watches.clear();
                            self.resync(&changes_tx, &mut watches, &mut watched);
                        }
                        Ok(_) => {}
                    }
                }
                // A changed path: arm the debounce when it is a watched project's repository
                // state, and never for the lock files git writes around its own writes.
                changed = changes_rx.recv() => {
                    let Some(path) = changed else {
                        break;
                    };
                    if !is_lock(&path) {
                        if let Some(project) = project_of(&watched, &path) {
                            debouncers
                                .entry(project)
                                .or_insert_with(|| Debouncer::new(QUIET))
                                .trigger(self.clock.now());
                        }
                    }
                }
                // The quiet window elapsed for at least one project: re-read the due ones.
                () = sleep_until(&self.clock, next_due) => {
                    let now = self.clock.now();
                    let Some(git) = self.git.upgrade() else {
                        break;
                    };
                    let mut due: Vec<ProjectId> = Vec::new();
                    debouncers.retain(|&project, debouncer| {
                        if debouncer.take_if_due(now) {
                            due.push(project);
                        }
                        debouncer.due_at().is_some()
                    });
                    for project in due {
                        let Some(root) = watched.get(&project).map(|w| w.root.clone()) else {
                            continue;
                        };
                        // Reading a repository runs an external tool, so it goes to the blocking
                        // pool: a slow or huge repository must never park a runtime worker.
                        let read = {
                            let git = git.clone();
                            run_blocking(move || git.refresh(project, &root)).await
                        };
                        match read {
                            Ok(changed) => {
                                retried.remove(&project);
                                if changed {
                                    self.bus.publish(DomainEvent::GitStatusChanged { project });
                                }
                            }
                            // The read failed — most likely it raced a write still holding the
                            // repository's lock. Re-arm it once; a second failure is left alone.
                            Err(_) => {
                                if retried.insert(project) {
                                    debouncers
                                        .entry(project)
                                        .or_insert_with(|| Debouncer::new(QUIET))
                                        .trigger(now);
                                } else {
                                    retried.remove(&project);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Dropping `watches` here stops every watch — the reactor leaves no OS watch behind.
        drop(watches);
    }

    /// Reconciles the per-project OS watches to the registry: a project already watched keeps
    /// its watches (no churn), a newly-opened one gains a non-recursive watch on its repository
    /// state plus a tree watch on the refs inside it, and a removed one has its handles dropped,
    /// releasing the OS resources. A failed registry read changes nothing — the next lifecycle
    /// event re-syncs.
    fn resync(
        &self,
        changes_tx: &mpsc::Sender<PathBuf>,
        watches: &mut HashMap<ProjectId, Vec<Box<dyn WatchHandle>>>,
        watched: &mut HashMap<ProjectId, Watched>,
    ) {
        let Ok(records) = self.projects.list() else {
            return;
        };
        watched.clear();
        let mut open: HashSet<ProjectId> = HashSet::new();
        for record in records {
            open.insert(record.id);
            let state_dir = record.root.join(STATE_DIR);
            watches.entry(record.id).or_insert_with(|| {
                vec![
                    self.watcher
                        .watch_dir(state_dir.clone(), changes_tx.clone()),
                    self.watcher
                        .watch(state_dir.join(REFS_DIR), changes_tx.clone()),
                ]
            });
            watched.insert(
                record.id,
                Watched {
                    root: record.root,
                    state_dir,
                },
            );
        }
        watches.retain(|project, _| open.contains(project));
    }
}

/// The project whose repository state `path` belongs to, if any.
fn project_of(watched: &HashMap<ProjectId, Watched>, path: &Path) -> Option<ProjectId> {
    watched
        .iter()
        .find(|(_, repo)| path.starts_with(&repo.state_dir))
        .map(|(&project, _)| project)
}

/// Whether `path` is one of the lock files git writes around its own writes.
fn is_lock(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == LOCK_EXTENSION)
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
