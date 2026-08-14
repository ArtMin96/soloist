//! The live-status trigger: a [`Clock`]-driven reactor that turns changes to a repository — to
//! its own state files and to the working tree beside them — into debounced status re-reads.
//!
//! The reactor watches two things per open project, because a status has two sources. The
//! repository state says what is committed and staged: the files directly inside `.git` (`HEAD`,
//! `index`, `packed-refs`, `MERGE_HEAD`, `FETCH_HEAD`) plus the `refs` tree, which is nested. The
//! working tree says what differs from it, and a file edited, added, or deleted there touches
//! nothing under `.git` — so the tree is watched too, as a tree, minus the directories whose
//! churn is never a change worth reading (the shared [`is_ignored`], so the build and dependency
//! trees are named in one place). Both sources feed **one** quiet window per project through the
//! shared [`crate::debounce::Debouncer`], so an operation that touches both — `git add` writing
//! the index beside the file it staged — coalesces into a single re-read of that project's status
//! through [`Git`]. It announces [`DomainEvent::GitStatusChanged`] only when the re-read differs
//! from what was already known, so churn that leaves the working tree looking the same wakes no
//! surface.
//!
//! Two rules keep it from fighting the tool it observes. The lock files git creates and removes
//! around every write are ignored: the write they guard reports separately when it lands, and
//! reacting to one only races a write still in progress. And a read that fails is retried
//! **once**, after another quiet window — a status run can still lose that race — so a
//! momentary failure corrects itself without the core polling or ever looping.
//!
//! The quiet window carries a ceiling ([`MAX_POSTPONE`]), because a tree that never goes quiet would
//! otherwise never be read: an agent writing file after file re-arms the window before it elapses,
//! and coalescing would turn into never refreshing at all.
//!
//! It watches a project's own root and `.git`; a project that is a subdirectory of a repository
//! keeps its status, and its own files still report, but the state above the watched root does
//! not.
//!
//! Like the other filesystem reactors it re-syncs on [`DomainEvent::ProjectOpened`] and
//! [`DomainEvent::ProjectRemoved`], holds the context weakly so it never keeps the app alive,
//! and ends when the bus closes.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{is_ignored, FileWatcher, WatchHandle, WatchPurpose, WatchStatus};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::projects::Projects;
use crate::supervision::run_blocking;
use crate::watch::WatchError;

use super::status::Git;

/// The quiet window a burst of changes is coalesced into before one status read. Long enough to
/// absorb the several files a single `git` invocation writes, short enough that a commit made in
/// a terminal beside the rail shows up as it happens.
///
/// One window serves the repository state and the working tree alike, and deliberately so: a
/// second window would mean a second debouncer per project, and an operation that touches both
/// sources — most of them do — would then read the status twice for one logical change. A burst
/// from the working tree is the larger of the two, but it is not what bounds the read rate: the
/// reactor awaits each read before it looks at the next due project, so a tree under continuous
/// change is re-read at most once per read, however many events it produces.
const QUIET: Duration = Duration::from_millis(100);

/// The longest a status read is postponed by changes that keep arriving. The quiet window alone has
/// no ceiling, and a tree under *continuous* change re-arms it before it ever elapses — an agent
/// writing two hundred files in a stream, a tool rewriting a directory the ignore list does not name
/// — so the rail would go on showing what the tree looked like before any of it started, for as long
/// as it lasted. Past this the read goes ahead anyway.
///
/// Long enough that an ordinary burst still coalesces into one read, short enough that a tree nobody
/// is going to stop changing still refreshes while it is being changed.
const MAX_POSTPONE: Duration = Duration::from_secs(1);

/// How many pending changed paths the watch channel buffers before the adapter's sends start
/// dropping. Bounded (no unbounded channel), and small on purpose: the paths are only ever used
/// to decide *that* a project changed, never which file did, so buffering a whole tree's burst
/// would cost memory for no extra information. Dropping is safe for the same reason — the burst
/// has already armed the debounce, the next change re-arms it, and the read it leads to reads the
/// working tree whole, so a status arrived at from a partial view of the burst is still the
/// status the tree actually has.
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

/// The watches held for one project. `refusal` is the first the OS gave while establishing them,
/// kept because a re-sync reports every open project's standing answer and does not re-establish
/// the watches it already holds.
struct Held {
    handles: Vec<Box<dyn WatchHandle>>,
    refusal: Option<WatchError>,
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
    status: Arc<WatchStatus>,
}

impl GitStatusWatchReactor {
    /// Builds a reactor over the file watcher and clock, watching the git context weakly (so it
    /// never keeps the app alive), subscribing to the bus for project lifecycle and the shutdown
    /// signal, and reporting what the OS refuses through the shared [`WatchStatus`].
    pub fn new(
        clock: Arc<dyn Clock>,
        watcher: Arc<dyn FileWatcher>,
        bus: &EventBus,
        git: Weak<Git>,
        projects: Arc<Projects>,
        status: Arc<WatchStatus>,
    ) -> Self {
        Self {
            clock,
            watcher,
            events: bus.subscribe(),
            bus: bus.clone(),
            git,
            projects,
            status,
        }
    }

    /// Runs the reactor until the bus closes (app shutdown) or the git context is dropped.
    pub async fn run(mut self) {
        let (changes_tx, mut changes_rx) = mpsc::channel(CHANGE_BUFFER);
        // The watch state, held for the reactor's lifetime. `watches` keeps each project's OS
        // watches alive: dropping a handle stops its watch (the bounded-resource contract), so
        // a handle lives exactly as long as its project is open. `watched` is what a change
        // event is matched against and where a re-read runs.
        let mut watches: HashMap<ProjectId, Held> = HashMap::new();
        let mut watched: HashMap<ProjectId, Watched> = HashMap::new();
        self.resync(&changes_tx, &mut watches, &mut watched).await;

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
                            self.resync(&changes_tx, &mut watches, &mut watched).await;
                        }
                        Ok(DomainEvent::ProjectRemoved { id }) => {
                            if let Some(git) = self.git.upgrade() {
                                git.forget(id);
                            }
                            debouncers.remove(&id);
                            retried.remove(&id);
                            self.resync(&changes_tx, &mut watches, &mut watched).await;
                        }
                        // A lag may have hidden an open whose directory was replaced, so rebuild
                        // every watch rather than trust the ones we hold.
                        Err(RecvError::Lagged(_)) => {
                            watches.clear();
                            self.resync(&changes_tx, &mut watches, &mut watched).await;
                        }
                        Ok(_) => {}
                    }
                }
                // A changed path: arm the debounce for every watched project whose status it is
                // part of, and never for the lock files git writes around its own writes.
                changed = changes_rx.recv() => {
                    let Some(path) = changed else {
                        break;
                    };
                    if !is_lock(&path) {
                        let now = self.clock.now();
                        for project in projects_of(&watched, &path) {
                            debouncers
                                .entry(project)
                                .or_insert_with(|| Debouncer::bounded(QUIET, MAX_POSTPONE))
                                .trigger(now);
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
                            // Armed at the clock's reading *now*, not the one taken before the
                            // read: a read that itself outlasted the quiet window would otherwise
                            // arm a deadline already past, and the retry would go straight back in
                            // with no quiet window at all — into the very race it is retrying.
                            Err(_) => {
                                if retried.insert(project) {
                                    debouncers
                                        .entry(project)
                                        .or_insert_with(|| Debouncer::bounded(QUIET, MAX_POSTPONE))
                                        .trigger(self.clock.now());
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
    /// state, a tree watch on the refs inside it, and a tree watch on its working tree, and a
    /// removed one has its handles dropped, releasing the OS resources. A failed registry read
    /// changes nothing — the next lifecycle event re-syncs.
    ///
    /// Every open project's standing answer — watched, or the refusal met establishing it — is
    /// reported to [`WatchStatus`], so a repository whose changes have stopped reporting says so
    /// rather than looking like one nobody is touching.
    async fn resync(
        &self,
        changes_tx: &mpsc::Sender<PathBuf>,
        watches: &mut HashMap<ProjectId, Held>,
        watched: &mut HashMap<ProjectId, Watched>,
    ) {
        let Ok(records) = self.projects.list() else {
            return;
        };
        watched.clear();
        let mut open: HashSet<ProjectId> = HashSet::new();
        let mut outcomes: Vec<(ProjectId, Option<WatchError>)> = Vec::new();
        for record in records {
            open.insert(record.id);
            let state_dir = record.root.join(STATE_DIR);
            let refusal = match watches.entry(record.id) {
                Entry::Occupied(slot) => slot.get().refusal,
                Entry::Vacant(slot) => {
                    slot.insert(
                        self.establish(&record.root, &state_dir, changes_tx.clone())
                            .await,
                    )
                    .refusal
                }
            };
            outcomes.push((record.id, refusal));
            watched.insert(
                record.id,
                Watched {
                    root: record.root,
                    state_dir,
                },
            );
        }
        watches.retain(|project, _| open.contains(project));
        self.status.resynced(WatchPurpose::GitStatus, &outcomes);
    }

    /// The watches one project's status needs: its repository state, the refs tree inside it, and
    /// its working tree.
    ///
    /// Registering them reads the filesystem — the working-tree watch walks the whole tree — so all
    /// three go to the blocking pool together: a large repository must never park a runtime worker
    /// while the OS enumerates its directories.
    ///
    /// Each is independent, so what can be watched is. A refused working-tree watch still leaves
    /// what version control itself writes reporting, which is why the repository-state watches are
    /// separate from the tree watch that spans them: the tree watch is the one an exhausted watch
    /// budget refuses first, and losing it must not take committing and staging down with it.
    ///
    /// Only the working tree's refusal is carried back to [`WatchStatus`], though every refusal is
    /// traced. A project that is not a repository has no `.git` to watch, so a state-dir refusal is
    /// the ordinary case rather than a loss — reporting it would put a notice on every project not
    /// under version control. The working-tree watch is the one every project has, and the one whose
    /// silence is indistinguishable from a tree nobody edits.
    async fn establish(
        &self,
        root: &Path,
        state_dir: &Path,
        changes: mpsc::Sender<PathBuf>,
    ) -> Held {
        let watcher = self.watcher.clone();
        let state_dir = state_dir.to_path_buf();
        let refs_dir = state_dir.join(REFS_DIR);
        let root = root.to_path_buf();
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
            Held { handles, refusal }
        })
        .await
    }
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

/// Every watched project whose status `path` is part of — through its repository state or its
/// working tree.
///
/// More than one can match, because projects nest: a repository opened inside another project's
/// tree shares its files, so a file changed there changes what both statuses say. Each is armed,
/// since each is a status of its own with a rail of its own — and a project whose status turns out
/// not to have changed announces nothing, so the extra read costs a subprocess and no more.
///
/// The working tree is matched by the project root minus the ignored directories, and `.git` is
/// one of them, so a repository-state path can only ever match a project through its `state_dir`.
/// That is what keeps the tree watch (which spans `.git` too, being recursive over the root) from
/// making one `.git` write look like two different changes.
fn projects_of<'a>(
    watched: &'a HashMap<ProjectId, Watched>,
    path: &'a Path,
) -> impl Iterator<Item = ProjectId> + 'a {
    watched
        .iter()
        .filter(move |(_, repo)| {
            path.starts_with(&repo.state_dir)
                || path
                    .strip_prefix(&repo.root)
                    .is_ok_and(|relative| !is_ignored(relative))
        })
        .map(|(&project, _)| project)
}

/// Whether `path` is one of the lock files git writes around its own writes.
fn is_lock(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == LOCK_EXTENSION)
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
