//! Filesystem watching over `notify`: the OS read behind the core's `FileWatcher`.
//!
//! For each watched directory, this registers a `notify` watcher (inotify on Linux) —
//! recursive for project roots, non-recursive for a single directory — and forwards every
//! created, modified, or removed absolute path to the core's reactors. The watcher delivers
//! events on its own OS thread, so it never blocks the async runtime; the callback only pushes
//! onto the bounded channel. All matching, the default ignores, and debouncing stay in the pure
//! core ([`soloist_core::WatchReactor`]) — the adapter reports raw changes, so every
//! testable decision lives in the core, not here.
//!
//! Establishing a watch blocks the calling thread until the backend has registered it. For a
//! recursive one that means walking the whole tree under the root and spending one inotify watch per
//! directory in it — unbounded work, so the core reaches those off a runtime worker; a non-recursive
//! one is a single registration and is not worth the hop.
//!
//! A watch the OS refuses comes back as a [`WatchError`] rather than as a handle that reports
//! nothing: silence is what a tree nobody touches looks like too, so a swallowed refusal would be a
//! subsystem that died without saying anything. `notify` stops a recursive registration at the first
//! directory it cannot watch, so an exhausted watch budget refuses the whole root rather than part of
//! it — which is why losing one root must not cost the others.
//!
//! [`NotifyFileWatcher::open`] backs the session-based half of the port: one `notify` watcher
//! instance registers every directory asked of it, non-recursively per directory, so the app holds
//! one inotify instance regardless of how many directories or projects it watches — where the
//! legacy [`FileWatcher::watch`]/[`FileWatcher::watch_dir`] above build a fresh instance per call.
//! Because registration is per directory rather than per tree, a newly created subdirectory is not
//! auto-added by `notify` the way a recursive registration would add it; the core registers it
//! itself once its own scan finds it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use notify::event::{ModifyKind, RenameMode};
use notify::{Config, ErrorKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use soloist_core::filewatch::{FileChange, FileChangeKind, WatchSession};
use soloist_core::{FileWatcher, WatchError, WatchHandle};
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
    fn watch(
        &self,
        root: PathBuf,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        start_watch(&root, RecursiveMode::Recursive, changes)
    }

    fn watch_dir(
        &self,
        dir: PathBuf,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        start_watch(&dir, RecursiveMode::NonRecursive, changes)
    }

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

/// Builds a watcher on `dir` at the given depth, or says why it could not be built — the backend
/// itself would not start, the OS is out of watches, or the directory is unreadable or gone.
fn start_watch(
    dir: &Path,
    mode: RecursiveMode,
    changes: mpsc::Sender<PathBuf>,
) -> Result<Box<dyn WatchHandle>, WatchError> {
    // Runs on notify's own delivery thread. Creations, modifications, and removals each leave the
    // tree different from before, so all three are reported; access events (a file merely opened,
    // read, or closed) change nothing and are not. A rename needs no case of its own — the
    // backend reports one as a modification of the name, so it arrives with the modifications.
    // `try_send` never blocks that thread, and a full channel drops the path harmlessly — the
    // burst already armed the consuming reactor's debounce, and the next change re-arms it.
    let forward = move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else {
            return;
        };
        if event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove() {
            for path in event.paths {
                let _ = changes.try_send(path);
            }
        }
    };

    let mut watcher =
        RecommendedWatcher::new(forward, Config::default()).map_err(|_| WatchError::Unavailable)?;
    watcher.watch(dir, mode).map_err(refusal)?;
    Ok(Box::new(NotifyWatchHandle { _watcher: watcher }))
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

/// A live `notify` watch. Dropping it stops the OS watch and releases its inotify
/// descriptors, so the reactor holds one per watched root for exactly as long as it watches.
struct NotifyWatchHandle {
    _watcher: RecommendedWatcher,
}

impl WatchHandle for NotifyWatchHandle {}

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
