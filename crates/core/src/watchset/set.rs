//! The stateful loop: one owner maintaining every open project's watches incrementally as
//! directories appear and vanish, and serving the three consumer reactors from a single
//! fan-out.
//!
//! Split across two files for size, both `impl ProjectWatchSet`: this one owns the type
//! vocabulary and the event loop that drives it; [`super::reconcile`] owns the planning half
//! (`resync`/`replan`) — "what should this project watch, and does it" is its own coherent
//! question, large enough on its own to want a file of its own.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{
    FileChange, FileChangeKind, FileWatcher, ScanRequest, WatchScanner, WatchSession, WatchStatus,
    DEFAULT_IGNORES,
};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::projects::Projects;
use crate::supervision::{run_blocking, supervise};
use crate::supervisor::Supervisor;
use crate::watch::{WatchLimit, WatchPurpose};

use super::registry::Registrations;

/// How many raw filesystem changes the adapter may have in flight before its sends start
/// dropping. Bounded, and a drop is self-healing (see [`Self::run_loop`]'s `dropped` handling),
/// so a burst larger than this loses no more than a prompt re-plan.
const CHANGE_BUFFER: usize = 1_024;

/// How many changed paths a lagging consumer (a reactor's `subscribe()` receiver) may fall
/// behind before it starts missing them. A `Lagged` receiver re-syncs its own state wholesale
/// (each reactor's own `resync`), so a drop here costs a wasted re-sync, not correctness.
///
/// Read only by [`ProjectWatchSet::new`], not yet called outside this module's own tests until
/// the composition root builds a set over it.
#[allow(dead_code)]
const FANOUT_CAPACITY: usize = 1_024;

/// The most paths one `Appeared` scan republishes onto the fan-out. A directory created and
/// immediately populated with more files than this republishes only the first — bounded so one
/// burst cannot flood every reactor's queue.
const MAX_APPEARED_REPLAY: usize = 4_096;

/// How long a run waits, after the adapter reports a dropped change, before re-planning every
/// open project from scratch. A dropped `Appeared` for a directory would otherwise leave that
/// subtree permanently unwatched with nothing to notice; this is the edge-triggered self-heal.
const RESCAN_QUIET: Duration = Duration::from_millis(500);

/// One open project's cached plan: what it is watched at, whether that came from watch-eligible
/// commands, and what had to be dropped to make it fit. Persists across re-syncs that do not
/// re-scan, so a re-sync that only retries a stuck refusal costs no filesystem walk.
pub(super) struct Registered {
    pub(super) root: PathBuf,
    /// The `restart_when_changed` globs of its currently watch-eligible commands — cached so a
    /// re-sync can tell "unchanged" from "needs a fresh scan" without re-reading the supervisor.
    pub(super) globs: Vec<String>,
    /// Every path the last plan wants watched, held or not — diffed against what is actually
    /// held on every re-sync, so a still-refused path is retried without re-scanning.
    pub(super) paths: HashSet<PathBuf>,
    /// The degradation the last plan met. Never carries a [`WatchLimit::Refused`] — that is
    /// recomputed fresh every re-sync from whether the working-tree root is currently held, and
    /// merged over this on report, so a refusal that clears is withdrawn on the very next
    /// re-sync without needing a fresh scan.
    pub(super) limit: HashMap<WatchPurpose, WatchLimit>,
}

/// Per-run mutable state: the live session, what is physically held, and each open project's
/// cached plan. Rebuilt from scratch on every restart — see the module doc for why holding any
/// of this across one would leave the app watching nothing while believing it watches
/// everything.
pub(super) struct RunState {
    pub(super) session: Option<Arc<dyn WatchSession>>,
    pub(super) registrations: Registrations,
    pub(super) projects: HashMap<ProjectId, Registered>,
}

/// The single owner of Soloist's filesystem watch registrations. See the module doc.
#[derive(Clone)]
pub struct ProjectWatchSet {
    pub(super) clock: Arc<dyn Clock>,
    pub(super) watcher: Arc<dyn FileWatcher>,
    pub(super) scanner: Arc<dyn WatchScanner>,
    pub(super) bus: EventBus,
    pub(super) projects: Arc<Projects>,
    pub(super) supervisor: Weak<Supervisor>,
    pub(super) status: Arc<WatchStatus>,
    fanout: broadcast::Sender<PathBuf>,
}

impl ProjectWatchSet {
    /// Builds a watch set over the given ports, watching the supervisor weakly (so it never
    /// keeps the app alive) and reporting through the shared [`WatchStatus`].
    ///
    /// Constructed by this module's own tests only until the composition root builds one over
    /// the real ports and spawns [`Self::run`].
    #[allow(dead_code)]
    pub(crate) fn new(
        clock: Arc<dyn Clock>,
        watcher: Arc<dyn FileWatcher>,
        scanner: Arc<dyn WatchScanner>,
        bus: &EventBus,
        projects: Arc<Projects>,
        supervisor: Weak<Supervisor>,
        status: Arc<WatchStatus>,
    ) -> Self {
        let (fanout, _rx) = broadcast::channel(FANOUT_CAPACITY);
        Self {
            clock,
            watcher,
            scanner,
            bus: bus.clone(),
            projects,
            supervisor,
            status,
            fanout,
        }
    }

    /// Subscribes to every changed path this set reports — what the config-reload, restart, and
    /// git-status reactors each consume in place of their own watcher.
    pub fn subscribe(&self) -> broadcast::Receiver<PathBuf> {
        self.fanout.subscribe()
    }

    /// Runs the watch set under panic isolation (see [`supervise`]), restarting it after a
    /// backoff if it dies — the session itself, or the scan it drives, can panic on a dead
    /// backend, and losing file watching silently for the rest of the process is exactly the
    /// failure this subsystem exists to avoid. Returned for the composition root to spawn once.
    pub async fn run(self) {
        let clock = self.clock.clone();
        supervise(clock, move || self.clone().run_loop()).await;
    }

    /// The loop itself. All per-run state is created here, never read from `self` — see the
    /// module doc's table of what lives where and why.
    async fn run_loop(self) {
        let mut events = self.bus.subscribe();
        let (changes_tx, mut changes_rx) = mpsc::channel::<FileChange>(CHANGE_BUFFER);
        let dropped = Arc::new(AtomicU64::new(0));
        let mut dropped_seen = 0u64;
        let mut rescan = Debouncer::new(RESCAN_QUIET);
        let mut state = RunState {
            session: None,
            registrations: Registrations::new(self.watcher.capacity()),
            projects: HashMap::new(),
        };

        self.resync(&mut state, &changes_tx, &dropped).await;

        loop {
            let next_due = rescan.due_at();
            tokio::select! {
                // The event bus drives re-syncs: a closed bus means the facade dropped, so
                // stop; a project opening or being removed, a `solo.yml` reload (which can add,
                // remove, or re-glob a watch-eligible command), or a lag that may have hidden
                // any of them means what to watch may have changed. Changes themselves arrive
                // on `changes_rx`, not here.
                result = events.recv() => {
                    match result {
                        Err(RecvError::Closed) => break,
                        Ok(DomainEvent::ProjectOpened { .. }
                            | DomainEvent::ProjectRemoved { .. }
                            | DomainEvent::ConfigChanged { .. })
                        | Err(RecvError::Lagged(_)) => {
                            self.resync(&mut state, &changes_tx, &dropped).await;
                        }
                        Ok(_) => {}
                    }
                }
                // A raw filesystem change: register a newly appeared directory, prune a
                // vanished one, and republish the path itself to every consumer regardless.
                changed = changes_rx.recv() => {
                    let Some(change) = changed else { break; };
                    self.handle_change(&mut state, change).await;
                }
                // The dropped-change debounce elapsed: re-plan every open project from
                // scratch, since a dropped `Appeared` may have hidden a subtree that would
                // otherwise stay unwatched forever.
                () = sleep_until(&self.clock, next_due) => {
                    let now = self.clock.now();
                    if rescan.take_if_due(now) {
                        self.resync(&mut state, &changes_tx, &dropped).await;
                    }
                }
            }

            // Edge-triggered self-heal: notice a change the adapter could not deliver and arm
            // the debounce above, rather than polling for it.
            let seen = dropped.load(Ordering::Relaxed);
            if seen != dropped_seen {
                dropped_seen = seen;
                rescan.trigger(self.clock.now());
            }
        }
        // Dropping `state` here drops the session, which releases every OS watch — the set
        // leaves no OS resource behind.
        drop(state);
    }

    /// Applies one raw change: registers a newly appeared directory (and replays what scanning
    /// it found), prunes a vanished one, and republishes the path itself regardless — a
    /// modification carries no registration change of its own, but every reactor still needs to
    /// hear about it.
    async fn handle_change(&self, state: &mut RunState, change: FileChange) {
        match change.kind {
            FileChangeKind::Appeared => {
                let owners: Vec<ProjectId> = change
                    .path
                    .parent()
                    .and_then(|parent| state.registrations.owners_of(parent))
                    .map(|owners| owners.iter().copied().collect())
                    .unwrap_or_default();
                for project in owners {
                    self.absorb_appeared(state, &change.path, project).await;
                }
            }
            FileChangeKind::Vanished => self.prune_vanished(state, &change.path),
            FileChangeKind::Modified => {}
        }
        let _ = self.fanout.send(change.path);
    }

    /// Scans a newly appeared path (off the runtime), registers every directory it found, and
    /// republishes every path it found — the race-closing rescan: a file written between
    /// `mkdir` and the directory's own registration is still found and still reaches the
    /// restart reactor, because the walk happens *after* creation.
    async fn absorb_appeared(&self, state: &mut RunState, path: &Path, project: ProjectId) {
        let Some(session) = state.session.clone() else {
            return;
        };
        let open_projects = state.projects.len().max(1);
        let share = state.registrations.share(open_projects);
        let ceiling = share
            .saturating_sub(state.registrations.held_by(project))
            .max(1);
        let scanner = self.scanner.clone();
        let ignored_names: Vec<String> = DEFAULT_IGNORES
            .iter()
            .map(|name| (*name).to_string())
            .collect();
        let request = ScanRequest {
            root: path.to_path_buf(),
            ignored_names,
            honour_repository_ignores: true,
            ceiling,
        };
        let scan = run_blocking(move || scanner.scan(request)).await;

        let mut replay = Vec::new();
        for found in scan.paths.into_iter().take(MAX_APPEARED_REPLAY) {
            if found.directory {
                if let Err(err) =
                    state
                        .registrations
                        .register(&found.path, project, false, session.as_ref())
                {
                    tracing::warn!(
                        path = %found.path.display(),
                        refusal = %err,
                        "a newly created directory could not be watched",
                    );
                } else if let Some(entry) = state.projects.get_mut(&project) {
                    entry.paths.insert(found.path.clone());
                }
            }
            replay.push(found.path);
        }
        for found_path in replay {
            let _ = self.fanout.send(found_path);
        }
    }

    /// Drops `path` and everything beneath it from every project that held it, releasing the
    /// registration (and refunding the budget) once its last owner is gone.
    fn prune_vanished(&self, state: &mut RunState, path: &Path) {
        let Some(session) = state.session.clone() else {
            return;
        };
        let doomed: Vec<PathBuf> = state
            .registrations
            .paths()
            .filter(|held| held.starts_with(path))
            .cloned()
            .collect();
        for doomed_path in doomed {
            let owners: Vec<ProjectId> = state
                .registrations
                .owners_of(&doomed_path)
                .map(|owners| owners.iter().copied().collect())
                .unwrap_or_default();
            for project in owners {
                state
                    .registrations
                    .release(&doomed_path, project, session.as_ref());
                if let Some(entry) = state.projects.get_mut(&project) {
                    entry.paths.remove(&doomed_path);
                }
            }
        }
    }

    /// Releases everything a now-closed project held.
    pub(super) fn release_project(&self, state: &mut RunState, project: ProjectId) {
        let Some(entry) = state.projects.remove(&project) else {
            return;
        };
        let Some(session) = state.session.clone() else {
            return;
        };
        for path in entry.paths {
            state
                .registrations
                .release(&path, project, session.as_ref());
        }
    }
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
