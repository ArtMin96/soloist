//! The file-watch restart policy: a [`Clock`]-driven reactor that turns watched-file changes
//! into debounced restarts.
//!
//! The reactor consumes raw change events from the [`FileWatcher`] port, matches each changed
//! path against the watch-eligible commands' globs (the pure [`super::policy`], with the
//! default ignores), coalesces a burst into a single restart with the shared
//! [`crate::debounce::Debouncer`], and routes the restart through the supervisor's existing
//! [`Supervisor::file_restart`] — so file-watch reuses one restart behaviour (the trust gate
//! and the crash-tracking reset) rather than reimplementing it. It establishes watches at
//! startup and re-syncs them on each [`DomainEvent::ProjectOpened`],
//! [`DomainEvent::ProjectRemoved`], and [`DomainEvent::ConfigChanged`], so a project opened
//! after launch is watched too, a removed project's OS watch is released, and a `solo.yml`
//! reload that re-globs or adds a command takes effect without a re-open. It holds a [`Weak`] reference to the supervisor and
//! ends when the event bus closes (app shutdown), like the crash reactor; command-only,
//! trusted-only, and running-only all follow from the watch targets and the restart gate.

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
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::supervision::run_blocking;
use crate::supervisor::Supervisor;
use crate::watch::{WatchError, WatchLimit, WatchOutcome, WatchPurpose};

use super::policy::{compile, WatchRule};
use super::status::WatchStatus;
use super::watcher::{FileWatcher, WatchHandle};

/// The quiet window a burst of changes is coalesced into before a restart fires. Long enough
/// to absorb an editor writing several files for one save, short enough to feel immediate.
const QUIET: Duration = Duration::from_millis(300);

/// How many pending changed paths the watch channel buffers before the adapter's sends start
/// dropping. Bounded (no unbounded channel); a dropped path is harmless — the burst it
/// belongs to has already armed the debounce, and the next change re-arms it.
const CHANGE_BUFFER: usize = 256;

/// Turns watched-file changes into debounced command restarts. Built once by the composition
/// root (via [`crate::facade::Facade::file_watch_loop`]) and spawned on the runtime.
pub struct WatchReactor {
    clock: Arc<dyn Clock>,
    watcher: Arc<dyn FileWatcher>,
    events: broadcast::Receiver<DomainEvent>,
    supervisor: Weak<Supervisor>,
    status: Arc<WatchStatus>,
}

impl WatchReactor {
    /// Builds a reactor over the file watcher and clock, watching the given supervisor weakly
    /// (so it never keeps the app alive), subscribing to the bus for the shutdown signal, and
    /// reporting what the OS refuses through the shared [`WatchStatus`].
    pub(crate) fn new(
        clock: Arc<dyn Clock>,
        watcher: Arc<dyn FileWatcher>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
        status: Arc<WatchStatus>,
    ) -> Self {
        Self {
            clock,
            watcher,
            events: bus.subscribe(),
            supervisor,
            status,
        }
    }

    /// Runs the reactor until the bus closes (app shutdown) or the supervisor is dropped.
    /// Establishes a watch per distinct watch-eligible project root, re-syncing whenever a
    /// project opens so a project added after startup is watched too, then debounces matching
    /// changes into restarts. Returned as a future for the composition root to spawn once.
    pub async fn run(mut self) {
        let Some(supervisor) = self.supervisor.upgrade() else {
            return;
        };
        let (changes_tx, mut changes_rx) = mpsc::channel(CHANGE_BUFFER);
        // The watch state, held for the reactor's lifetime. `watches` keeps each root's OS
        // watch alive: dropping a handle stops its watch (the bounded-resource contract), so
        // a handle lives exactly as long as its root has watch-eligible commands. `resync`
        // rebuilds both from the current watch targets — once now, then again on each
        // project open or removal.
        let mut rules: Vec<WatchRule> = Vec::new();
        let mut watches: HashMap<PathBuf, Box<dyn WatchHandle>> = HashMap::new();
        self.resync(&supervisor, &changes_tx, &mut rules, &mut watches)
            .await;
        drop(supervisor);

        let mut debouncers: HashMap<ProcessId, Debouncer> = HashMap::new();
        loop {
            let next_due = debouncers.values().filter_map(Debouncer::due_at).min();
            tokio::select! {
                // The event bus drives two things: a closed bus means the facade dropped, so
                // stop; a project opening or being removed, a `solo.yml` reload (which can add,
                // remove, or re-glob a watch-eligible command), or a lag that may have hidden
                // any of them means the watch-eligible command set or its globs changed, so
                // re-sync the watches. Changes themselves arrive on `changes_rx`, not here.
                result = self.events.recv() => {
                    match result {
                        Err(RecvError::Closed) => break,
                        Ok(DomainEvent::ProjectOpened { .. }
                            | DomainEvent::ProjectRemoved { .. }
                            | DomainEvent::ConfigChanged { .. })
                        | Err(RecvError::Lagged(_)) => {
                            let Some(supervisor) = self.supervisor.upgrade() else {
                                break;
                            };
                            self.resync(&supervisor, &changes_tx, &mut rules, &mut watches)
                                .await;
                        }
                        Ok(_) => {}
                    }
                }
                // A changed path: arm the debounce for every command whose globs it matches.
                changed = changes_rx.recv() => {
                    let Some(path) = changed else {
                        break;
                    };
                    let now = self.clock.now();
                    for id in rules.iter().filter(|rule| rule.matches(&path)).map(|rule| rule.id) {
                        debouncers
                            .entry(id)
                            .or_insert_with(|| Debouncer::new(QUIET))
                            .trigger(now);
                    }
                }
                // The quiet window elapsed for at least one command: restart the due ones.
                () = sleep_until(&self.clock, next_due) => {
                    let now = self.clock.now();
                    let Some(supervisor) = self.supervisor.upgrade() else {
                        break;
                    };
                    let mut due: Vec<ProcessId> = Vec::new();
                    debouncers.retain(|&id, debouncer| {
                        if debouncer.take_if_due(now) {
                            due.push(id);
                        }
                        debouncer.due_at().is_some()
                    });
                    for id in due {
                        supervisor.file_restart(id);
                    }
                }
            }
        }
        // Dropping `watches` here stops every watch — the reactor leaves no OS watch behind.
        drop(watches);
    }

    /// Rebuilds the match rules from the current watch-eligible commands and reconciles the
    /// per-root OS watches to them: a root already watched keeps its existing watch (no
    /// duplicate, no churn), a newly-seen root gains one, and a root with no remaining
    /// watch-eligible command — its project removed or its commands gone — has its watch
    /// dropped, which releases the OS resources. The rules are rebuilt wholesale so a
    /// command that is gone simply drops out of matching.
    ///
    /// Registering a watch walks the tree under its root, so it goes to the blocking pool: a large
    /// project must never park a runtime worker while the OS enumerates its directories. A root the
    /// OS refuses is left out of the watch set rather than recorded as watched, so it is asked for
    /// again on the next re-sync and a refusal that has since cleared is not permanent — and it is
    /// reported to [`WatchStatus`], because a watch that yields no events looks exactly like a
    /// project nobody edits, so a refusal nobody is told about is a restart trigger that stopped
    /// working in silence.
    ///
    /// Every eligible project is reported each time, watched or refused, so the status can withdraw
    /// a refusal for a project that is no longer watched at all.
    async fn resync(
        &self,
        supervisor: &Supervisor,
        changes_tx: &mpsc::Sender<PathBuf>,
        rules: &mut Vec<WatchRule>,
        watches: &mut HashMap<PathBuf, Box<dyn WatchHandle>>,
    ) {
        rules.clear();
        let mut desired: HashSet<PathBuf> = HashSet::new();
        let mut outcomes: Vec<WatchOutcome> = Vec::new();
        for target in supervisor.watch_targets() {
            let Some(set) = compile(&target.globs) else {
                continue;
            };
            if desired.insert(target.project_root.clone()) {
                let refusal = match watches.entry(target.project_root.clone()) {
                    Entry::Occupied(_) => None,
                    Entry::Vacant(slot) => {
                        match self
                            .establish(&target.project_root, changes_tx.clone())
                            .await
                        {
                            Ok(handle) => {
                                slot.insert(handle);
                                None
                            }
                            Err(refusal) => Some(refusal),
                        }
                    }
                };
                outcomes.push(WatchOutcome {
                    project: target.project,
                    limit: refusal.map(WatchLimit::Refused),
                });
            }
            rules.push(WatchRule::new(target.id, target.project_root, set));
        }
        watches.retain(|root, _| desired.contains(root));
        self.status.resynced(WatchPurpose::Restarts, &outcomes);
    }

    /// A tree watch on `root`, registered off the runtime, or the refusal the OS gave — traced
    /// here so the log carries the path, and returned so the caller can report it.
    async fn establish(
        &self,
        root: &Path,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        let watcher = self.watcher.clone();
        let root = root.to_path_buf();
        run_blocking(move || {
            watcher.watch(root.clone(), changes).inspect_err(|refusal| {
                tracing::warn!(
                    path = %root.display(),
                    %refusal,
                    "a project's watched files will not restart anything: it could not be watched",
                );
            })
        })
        .await
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
