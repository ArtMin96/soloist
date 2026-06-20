//! The file-watch domain's own driven port: a recursive filesystem watcher that reports
//! the changes which may restart a command. The adapter (`crates/sys`, over `notify`)
//! watches the OS; the core never touches the filesystem here.

use std::path::PathBuf;

use tokio::sync::mpsc;

/// Watches project directories for the filesystem changes that drive file-watch restarts.
///
/// An implementation watches a project root **recursively** for create/modify events and
/// forwards each changed **absolute** path to the `changes` channel. All matching,
/// debouncing, and restarting is the pure policy's ([`super::WatchReactor`]) — the adapter
/// only reports raw changes, so every testable decision stays in the core.
pub trait FileWatcher: Send + Sync {
    /// Begins watching `root` recursively, forwarding each changed absolute path to `changes`
    /// until the returned [`WatchHandle`] is dropped (which stops the watch and releases its
    /// OS resources — the bounded-resource contract). Best-effort: an unwatchable root simply
    /// yields no events rather than failing the core.
    fn watch(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle>;
}

/// A live filesystem watch. Dropping it stops the watch and frees its OS resources, so the
/// reactor holds one per watched root for exactly as long as it watches that root.
pub trait WatchHandle: Send + Sync {}

/// A [`FileWatcher`] that watches nothing — the default until the OS adapter is wired
/// (headless tools, tests that do not exercise watching). The reactor then never restarts.
#[derive(Clone, Copy, Default)]
pub struct NoopFileWatcher;

impl FileWatcher for NoopFileWatcher {
    fn watch(&self, _root: PathBuf, _changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        Box::new(NoopWatchHandle)
    }
}

/// The [`WatchHandle`] for a no-op watch — its drop stops nothing.
#[derive(Clone, Copy, Default)]
pub struct NoopWatchHandle;

impl WatchHandle for NoopWatchHandle {}
