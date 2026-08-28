//! Filesystem watching over `notify`: the OS read behind the core's `FileWatcher`.
//!
//! [`NotifyFileWatcher::open`] backs the session-based port: one `notify` watcher instance
//! registers every directory asked of it, non-recursively per directory, so the app holds one
//! inotify instance regardless of how many directories or projects it watches. The watcher
//! delivers events on its own OS thread, so it never blocks the async runtime; the callback only
//! pushes onto the bounded channel. All matching, the default ignores, planning, and debouncing
//! stay in the pure core (`soloist_core::watchset::ProjectWatchSet`, `soloist_core::WatchReactor`)
//! — the adapter reports raw changes, so every testable decision lives in the core, not here.
//! Because registration is per directory rather than per tree, a newly created subdirectory is
//! not auto-added by `notify` the way a recursive registration would add it; the core registers
//! it itself once its own scan finds it.
//!
//! Registering a directory blocks the calling thread until the backend has registered it — a
//! single, bounded registration, so the core reaches it directly rather than off the runtime;
//! only the whole-tree scans that plan *what* to register walk unbounded work, and those are the
//! core's own concern.
//!
//! A directory the OS refuses comes back as a [`WatchError`] rather than succeeding silently:
//! silence is what a tree nobody touches looks like too, so a swallowed refusal would be a
//! subsystem that died without saying anything.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use notify::event::{ModifyKind, RenameMode};
use notify::{Config, ErrorKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use soloist_core::filewatch::{FileChange, FileChangeKind, WatchSession};
use soloist_core::{FileWatcher, WatchError};
use tokio::sync::mpsc;

/// Watches directories via `notify`, forwarding create/modify/remove paths to the core's watch
/// reactors.
#[derive(Clone, Copy, Default)]
pub struct NotifyFileWatcher;

impl NotifyFileWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl FileWatcher for NotifyFileWatcher {
    fn open(
        &self,
        changes: mpsc::Sender<FileChange>,
        dropped: Arc<AtomicU64>,
    ) -> Result<Arc<dyn WatchSession>, WatchError> {
        // Runs on notify's own delivery thread, once per registered path rather than once per
        // session — `try_send` never blocks it. A change that could not be sent (a full or
        // closed channel) is not silently lost: the caller notices `dropped` moved and re-plans.
        let forward = move |result: notify::Result<notify::Event>| {
            let Ok(event) = result else {
                return;
            };
            let Some(kind) = classify(&event.kind) else {
                return;
            };
            for path in event.paths {
                if changes.try_send(FileChange { path, kind }).is_err() {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        };

        let watcher = RecommendedWatcher::new(forward, Config::default())
            .map_err(|_| WatchError::Unavailable)?;
        Ok(Arc::new(NotifyWatchSession {
            watcher: Mutex::new(watcher),
        }))
    }

    fn capacity(&self) -> Option<usize> {
        let raw = std::fs::read_to_string("/proc/sys/fs/inotify/max_user_watches").ok()?;
        raw.trim().parse().ok()
    }
}

/// Maps one `notify` event to what the session-based port reports for it, or `None` for an event
/// the core has nothing to do with. Verified exhaustively against `notify-8.2.0`'s
/// `notify_types::event::EventKind` (no `non_exhaustive`, so every variant is matched here rather
/// than caught by a wildcard).
///
/// Two cases are not the obvious mapping. `Modify(Name(RenameMode::From))` is a **disappearance**,
/// not a modification: `notify` has already dropped its own registration for the path that moved
/// away, so treating it as anything but [`FileChangeKind::Vanished`] would leave the core holding
/// a registration for a directory that no longer exists and never refund its watch budget.
/// `Modify(Name(RenameMode::Both))` is dropped entirely rather than mapped: `notify` emits the
/// `From` and `To` halves of a rename first and *then* a third summary event carrying both paths,
/// so acting on the summary too would prune and re-scan the same rename twice.
fn classify(kind: &EventKind) -> Option<FileChangeKind> {
    match kind {
        EventKind::Create(_) => Some(FileChangeKind::Appeared),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => Some(FileChangeKind::Appeared),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => Some(FileChangeKind::Vanished),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => None,
        EventKind::Modify(
            ModifyKind::Name(RenameMode::Any | RenameMode::Other)
            | ModifyKind::Data(_)
            | ModifyKind::Metadata(_)
            | ModifyKind::Any
            | ModifyKind::Other,
        ) => Some(FileChangeKind::Modified),
        EventKind::Remove(_) => Some(FileChangeKind::Vanished),
        EventKind::Access(_) | EventKind::Any | EventKind::Other => None,
    }
}

/// What the backend's refusal means to the core. A recursive watch registers one per directory it
/// walks and stops at the first the OS will not give it, so an exhausted budget is the failure a
/// large tree actually meets — and the one the user can do something about.
fn refusal(err: notify::Error) -> WatchError {
    match err.kind {
        ErrorKind::MaxFilesWatch => WatchError::BudgetExhausted,
        _ => WatchError::Unwatchable,
    }
}

/// The [`WatchSession`] [`NotifyFileWatcher::open`] returns: one `notify` watcher instance shared
/// by every directory registered through it. Dropping it drops the watcher, releasing every
/// inotify descriptor it holds at once.
///
/// `notify`'s own registration path (`watch_inner`/`unwatch_inner`) `unwrap()`s a channel send and
/// receive to its event-loop thread, so a dead event-loop thread panics *inside* the call to
/// `watch`/`unwatch` below — while this lock is held — and poisons it. Every lock acquisition
/// therefore recovers a poisoned guard rather than propagating the panic into every later
/// registration, the same idiom this crate already uses in `metrics::ProcMetricsProbe::sample`.
struct NotifyWatchSession {
    watcher: Mutex<RecommendedWatcher>,
}

impl WatchSession for NotifyWatchSession {
    fn watch_dir(&self, dir: &Path) -> Result<(), WatchError> {
        self.watcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .watch(dir, RecursiveMode::NonRecursive)
            .map_err(refusal)
    }

    fn watch_tree(&self, root: &Path) -> Result<(), WatchError> {
        self.watcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .watch(root, RecursiveMode::Recursive)
            .map_err(refusal)
    }

    fn unwatch(&self, path: &Path) {
        // Best-effort: `notify` returns `WatchNotFound` for a path already gone or never
        // registered, which is not a failure the caller needs to hear about.
        let _ = self
            .watcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .unwatch(path);
    }
}
