//! Filesystem watching over `notify`: the OS read behind the core's `FileWatcher`.
//!
//! For each watched directory, this registers a `notify` watcher (inotify on Linux) —
//! recursive for project roots, non-recursive for a single directory — and forwards every
//! created or modified absolute path to the core's reactors. The watcher delivers events on
//! its own OS thread, so it never blocks the async runtime; the callback only pushes onto
//! the bounded channel. All matching, the default ignores, and debouncing stay in the pure
//! core ([`soloist_core::WatchReactor`]) — the adapter reports raw changes, so every
//! testable decision lives in the core, not here.

use std::path::{Path, PathBuf};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use soloist_core::{FileWatcher, NoopWatchHandle, WatchHandle};
use tokio::sync::mpsc;

/// Watches directories via `notify`, forwarding create/modify paths to the core's watch
/// reactors. Best-effort: a directory that cannot be watched simply yields no events rather
/// than failing the core (the port's contract).
#[derive(Clone, Copy, Default)]
pub struct NotifyFileWatcher;

impl NotifyFileWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl FileWatcher for NotifyFileWatcher {
    fn watch(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        start_watch(&root, RecursiveMode::Recursive, changes)
    }

    fn watch_dir(&self, dir: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        start_watch(&dir, RecursiveMode::NonRecursive, changes)
    }
}

/// Builds a watcher on `dir` at the given depth, or a no-op handle if the backend or the
/// watch could not be established (an unreadable or vanished directory) — the core then
/// sees no events for it, never an error.
fn start_watch(
    dir: &Path,
    mode: RecursiveMode,
    changes: mpsc::Sender<PathBuf>,
) -> Box<dyn WatchHandle> {
    // Runs on notify's own delivery thread. Only create/modify carry a real edit
    // (remove/access are not restart or reload triggers); `try_send` never blocks that
    // thread, and a full channel drops the path harmlessly — the burst already armed the
    // consuming reactor's debounce, and the next change re-arms it.
    let forward = move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else {
            return;
        };
        if event.kind.is_create() || event.kind.is_modify() {
            for path in event.paths {
                let _ = changes.try_send(path);
            }
        }
    };

    let watcher = RecommendedWatcher::new(forward, Config::default())
        .ok()
        .and_then(|mut watcher| watcher.watch(dir, mode).ok().map(|()| watcher));
    match watcher {
        Some(watcher) => Box::new(NotifyWatchHandle { _watcher: watcher }),
        None => Box::new(NoopWatchHandle),
    }
}

/// A live `notify` watch. Dropping it stops the OS watch and releases its inotify
/// descriptors, so the reactor holds one per watched root for exactly as long as it watches.
struct NotifyWatchHandle {
    _watcher: RecommendedWatcher,
}

impl WatchHandle for NotifyWatchHandle {}
