//! The `solo.yml` sync trigger: a [`Clock`]-driven reactor that turns external edits of an
//! open project's config file into debounced reloads.
//!
//! The reactor holds one **non-recursive** OS watch per open project root (via the
//! [`FileWatcher`] port — one descriptor per project, whatever the tree's size), matches
//! each reported path against the roots' `solo.yml` locations, coalesces an editor's save
//! burst with the shared [`crate::debounce::Debouncer`], and routes the reload through
//! [`ProjectService::reload`] — the same reconcile the HTTP `reload` endpoint drives, so an
//! external edit and an explicit reload are one behaviour. The sync engine underneath
//! hash-diffs the file (a byte-identical rewrite is a no-op), refreshes its hash on the
//! app's own writes (so a self-write never re-syncs), and announces
//! [`DomainEvent::ConfigChanged`] with the trust review the UI's dialog renders. Watches
//! re-sync on [`DomainEvent::ProjectOpened`] / [`DomainEvent::ProjectRemoved`], so a
//! project opened after launch is watched and a removed project's watch is released. Like
//! the file-watch reactor, it holds the supervisor weakly and ends when the bus closes.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::config::{config_path, ConfigEngine};
use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{FileWatcher, WatchHandle};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::supervisor::Supervisor;

use super::{ProjectService, Projects};

/// The quiet window a burst of config-file events is coalesced into before one reload.
/// Long enough to absorb an editor's save sequence (write + rename + metadata), short
/// enough that the trust review feels immediate.
const QUIET: Duration = Duration::from_millis(300);

/// How many pending changed paths the watch channel buffers before the adapter's sends
/// start dropping. Each watch is one non-recursive directory, so traffic is light; a
/// dropped path is harmless — the burst it belongs to has already armed the debounce.
const CHANGE_BUFFER: usize = 64;

/// Turns external `solo.yml` edits into debounced project reloads. Built once by the
/// composition root (via [`crate::facade::Facade::config_watch_loop`]) and spawned on the
/// runtime.
pub struct ConfigWatchReactor {
    clock: Arc<dyn Clock>,
    watcher: Arc<dyn FileWatcher>,
    events: broadcast::Receiver<DomainEvent>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
    projects: Arc<Projects>,
    config: Arc<ConfigEngine>,
}

impl ConfigWatchReactor {
    /// Builds a reactor over the file watcher and clock, sharing the contexts a reload
    /// spans, watching the supervisor weakly (so it never keeps the app alive) and
    /// subscribing to the bus for project lifecycle and the shutdown signal.
    pub fn new(
        clock: Arc<dyn Clock>,
        watcher: Arc<dyn FileWatcher>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
        projects: Arc<Projects>,
        config: Arc<ConfigEngine>,
    ) -> Self {
        Self {
            clock,
            watcher,
            events: bus.subscribe(),
            bus: bus.clone(),
            supervisor,
            projects,
            config,
        }
    }

    /// Runs the reactor until the bus closes (app shutdown) or the supervisor is dropped.
    /// Establishes a watch per open project root, re-syncing whenever a project opens or is
    /// removed, then debounces `solo.yml` changes into reloads.
    pub async fn run(mut self) {
        let (changes_tx, mut changes_rx) = mpsc::channel(CHANGE_BUFFER);
        // One watch per open project, alive exactly as long as its project is (dropping a
        // handle stops its watch — the bounded-resource contract), plus the config-file
        // paths those watches can report. `resync` reconciles both to the registry — once
        // now, then again on each project open or removal.
        let mut watches: HashMap<ProjectId, Box<dyn WatchHandle>> = HashMap::new();
        let mut config_paths: HashMap<PathBuf, ProjectId> = HashMap::new();
        self.resync(&changes_tx, &mut watches, &mut config_paths);

        let mut debouncers: HashMap<ProjectId, Debouncer> = HashMap::new();
        loop {
            let next_due = debouncers.values().filter_map(Debouncer::due_at).min();
            tokio::select! {
                // The event bus drives two things: a closed bus means the facade dropped, so
                // stop; a project opening or being removed (or a lag that may have hidden
                // either) means the watched-root set changed, so re-sync. Config-file changes
                // themselves arrive on `changes_rx`, not here.
                result = self.events.recv() => {
                    match result {
                        Err(RecvError::Closed) => break,
                        // A project open drops that project's existing watch first, forcing a
                        // fresh one: opening the same path can mean the directory was replaced
                        // (deleted and recreated — a new inode), which silently invalidates the
                        // OS watch; re-establishing it on every open keeps the watch tracking the
                        // live directory rather than a vanished one.
                        Ok(DomainEvent::ProjectOpened { id }) => {
                            watches.remove(&id);
                            self.resync(&changes_tx, &mut watches, &mut config_paths);
                        }
                        Ok(DomainEvent::ProjectRemoved { .. }) => {
                            self.resync(&changes_tx, &mut watches, &mut config_paths);
                        }
                        // A lag may have hidden an open whose directory was replaced, so rebuild
                        // every watch rather than trust the ones we hold.
                        Err(RecvError::Lagged(_)) => {
                            watches.clear();
                            self.resync(&changes_tx, &mut watches, &mut config_paths);
                        }
                        Ok(_) => {}
                    }
                }
                // A changed path: arm the debounce when it is an open project's `solo.yml`.
                changed = changes_rx.recv() => {
                    let Some(path) = changed else {
                        break;
                    };
                    if let Some(&project) = config_paths.get(&path) {
                        debouncers
                            .entry(project)
                            .or_insert_with(|| Debouncer::new(QUIET))
                            .trigger(self.clock.now());
                    }
                }
                // The quiet window elapsed for at least one project: reload the due ones.
                () = sleep_until(&self.clock, next_due) => {
                    let now = self.clock.now();
                    let Some(supervisor) = self.supervisor.upgrade() else {
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
                        // A failed reload is dropped, not fatal: a mid-edit save can be
                        // invalid YAML (the config keeps its last good state and the next
                        // save re-triggers), and a project removed while debouncing is
                        // simply unknown. An optional subsystem never crashes the core.
                        let _ = ProjectService::new(
                            &self.projects,
                            &self.config,
                            &supervisor,
                            &self.bus,
                        )
                        .reload(project);
                    }
                }
            }
        }
        // Dropping `watches` here stops every watch — the reactor leaves no OS watch behind.
        drop(watches);
    }

    /// Reconciles the per-project OS watches to the registry: a project already watched
    /// keeps its watch (no churn), a newly-opened one gains a non-recursive watch on its
    /// root, and a removed one has its watch dropped, releasing the OS resources. The
    /// config-path index is rebuilt wholesale so a removed project's `solo.yml` simply
    /// drops out of matching. A failed registry read changes nothing — the next lifecycle
    /// event re-syncs.
    fn resync(
        &self,
        changes_tx: &mpsc::Sender<PathBuf>,
        watches: &mut HashMap<ProjectId, Box<dyn WatchHandle>>,
        config_paths: &mut HashMap<PathBuf, ProjectId>,
    ) {
        let Ok(records) = self.projects.list() else {
            return;
        };
        config_paths.clear();
        let mut open: HashSet<ProjectId> = HashSet::new();
        for record in records {
            open.insert(record.id);
            config_paths.insert(config_path(&record.root), record.id);
            watches
                .entry(record.id)
                .or_insert_with(|| self.watcher.watch_dir(record.root, changes_tx.clone()));
        }
        watches.retain(|project, _| open.contains(project));
    }
}

#[cfg(test)]
#[path = "config_watch_tests.rs"]
mod tests;
