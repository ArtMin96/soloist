//! The live-status trigger: a [`Clock`]-driven reactor that turns changes to a repository — to
//! its own state files and to the working tree beside them — into debounced status re-reads.
//!
//! Where those changes come from is [`super::watched`]'s: it holds the watches and decides which
//! projects a changed path belongs to. This module decides when one is worth reading. Both sources
//! feed **one** quiet window per project through the shared [`crate::debounce::Debouncer`], so an
//! operation that touches both — `git add` writing the index beside the file it staged — coalesces
//! into a single re-read of that project's status through [`Git`]. It announces
//! [`DomainEvent::GitStatusChanged`] only when the re-read differs from what was already known, so
//! churn that leaves the working tree looking the same wakes no surface.
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
//! Like the other filesystem reactors it re-syncs on [`DomainEvent::ProjectOpened`] and
//! [`DomainEvent::ProjectRemoved`], holds the context weakly so it never keeps the app alive,
//! and ends when the bus closes.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{FileWatcher, WatchStatus};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::projects::Projects;
use crate::supervision::run_blocking;
use crate::watch::{WatchOutcome, WatchPurpose};

use super::status::Git;
use super::watched::{is_lock, Watches};

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
    pub(crate) fn new(
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
        // The watch set, held for the reactor's lifetime: it keeps each project's OS watches alive,
        // and `resync` reconciles it to the registry — once now, then again on each project open or
        // removal.
        let mut watches = Watches::new(self.watcher.clone(), changes_tx);
        self.resync(&mut watches).await;

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
                            watches.release(id);
                            self.resync(&mut watches).await;
                        }
                        Ok(DomainEvent::ProjectRemoved { id }) => {
                            if let Some(git) = self.git.upgrade() {
                                git.forget(id);
                            }
                            debouncers.remove(&id);
                            retried.remove(&id);
                            self.resync(&mut watches).await;
                        }
                        // A lag may have hidden an open whose directory was replaced, so rebuild
                        // every watch rather than trust the ones we hold.
                        Err(RecvError::Lagged(_)) => {
                            watches.release_all();
                            self.resync(&mut watches).await;
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
                        for project in watches.projects_of(&path) {
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
                        let Some(root) = watches.root_of(project) else {
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

    /// Reconciles the watches to the registry: a newly-opened project gains them, an already
    /// watched one keeps the ones it holds, and a removed one has its handles dropped, releasing
    /// the OS resources. A failed registry read changes nothing — the next lifecycle event
    /// re-syncs.
    ///
    /// Every open project's standing answer — watched, or the refusal met establishing it — is
    /// reported to [`WatchStatus`], so a repository whose changes have stopped reporting says so
    /// rather than looking like one nobody is touching.
    async fn resync(&self, watches: &mut Watches) {
        let Ok(records) = self.projects.list() else {
            return;
        };
        let mut open: HashSet<ProjectId> = HashSet::new();
        let mut outcomes: Vec<WatchOutcome> = Vec::new();
        for record in records {
            open.insert(record.id);
            outcomes.push(watches.establish(record.id, record.root).await);
        }
        watches.retain(&open);
        self.status.resynced(WatchPurpose::GitStatus, &outcomes);
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
