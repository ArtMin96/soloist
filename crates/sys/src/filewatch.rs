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

use std::path::{Path, PathBuf};

use notify::{Config, ErrorKind, RecommendedWatcher, RecursiveMode, Watcher};
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
