//! Recursive filesystem watching over `notify`: the OS read behind the core's `FileWatcher`.
//!
//! For each watched project root, this registers a recursive `notify` watcher (inotify on
//! Linux) and forwards every created or modified absolute path to the core's reactor. The
//! watcher delivers events on its own OS thread, so it never blocks the async runtime; the
//! callback only pushes onto the bounded channel. All matching, the default ignores, and
//! debouncing stay in the pure core ([`soloist_core::WatchReactor`]) — the adapter reports
//! raw changes, so every testable decision lives in the core, not here.

use std::path::{Path, PathBuf};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use soloist_core::{FileWatcher, NoopWatchHandle, WatchHandle};
use tokio::sync::mpsc;

/// Watches project roots recursively via `notify`, forwarding create/modify paths to the
/// file-watch reactor. Best-effort: a root that cannot be watched simply yields no events
/// rather than failing the core (the port's contract).
#[derive(Clone, Copy, Default)]
pub struct NotifyFileWatcher;

impl NotifyFileWatcher {
    pub fn new() -> Self {
        Self
    }
}

impl FileWatcher for NotifyFileWatcher {
    fn watch(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        // Runs on notify's own delivery thread. Only create/modify carry a real edit
        // (remove/access are not restart triggers); `try_send` never blocks that thread,
        // and a full channel drops the path harmlessly — the burst already armed the
        // reactor's debounce, and the next change re-arms it.
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

        match start_watch(&root, forward) {
            Some(watcher) => Box::new(NotifyWatchHandle { _watcher: watcher }),
            None => Box::new(NoopWatchHandle),
        }
    }
}

/// Builds a recursive watcher on `root`, or `None` if the backend or the recursive watch
/// could not be established (an unreadable or vanished root) — the core then sees no events
/// for it, never an error.
fn start_watch(
    root: &Path,
    forward: impl FnMut(notify::Result<notify::Event>) + Send + 'static,
) -> Option<RecommendedWatcher> {
    let mut watcher = RecommendedWatcher::new(forward, Config::default()).ok()?;
    watcher.watch(root, RecursiveMode::Recursive).ok()?;
    Some(watcher)
}

/// A live `notify` watch. Dropping it stops the OS watch and releases its inotify
/// descriptors, so the reactor holds one per watched root for exactly as long as it watches.
struct NotifyWatchHandle {
    _watcher: RecommendedWatcher,
}

impl WatchHandle for NotifyWatchHandle {}
