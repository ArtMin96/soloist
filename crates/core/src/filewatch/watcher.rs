//! The file-watch domain's own driven port: a filesystem watcher that reports the changes
//! which may restart a command or reload a project's config. The adapter (`crates/sys`,
//! over `notify`) watches the OS; the core never touches the filesystem here.

use std::path::PathBuf;

use tokio::sync::mpsc;

/// Watches project directories for the filesystem changes that drive file-watch restarts
/// and `solo.yml` reloads.
///
/// An implementation watches a directory for create/modify events and forwards each changed
/// **absolute** path to the `changes` channel. All matching, debouncing, restarting, and
/// reloading is the consuming reactor's ([`super::WatchReactor`],
/// [`crate::projects::ConfigWatchReactor`]) — the adapter only reports raw changes, so
/// every testable decision stays in the core.
pub trait FileWatcher: Send + Sync {
    /// Begins watching `root` recursively, forwarding each changed absolute path to `changes`
    /// until the returned [`WatchHandle`] is dropped (which stops the watch and releases its
    /// OS resources — the bounded-resource contract). Best-effort: an unwatchable root simply
    /// yields no events rather than failing the core.
    fn watch(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle>;

    /// Begins watching the single directory `dir` — **non-recursive**, so only its direct
    /// children report — with the same channel, handle, and best-effort contract as
    /// [`Self::watch`]. For a file at a fixed, known location (a project root's `solo.yml`),
    /// where a recursive tree watch would spend an inotify descriptor per subdirectory to
    /// observe one file.
    fn watch_dir(&self, dir: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle>;
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

    fn watch_dir(&self, _dir: PathBuf, _changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        Box::new(NoopWatchHandle)
    }
}

/// The [`WatchHandle`] for a no-op watch — its drop stops nothing.
#[derive(Clone, Copy, Default)]
pub struct NoopWatchHandle;

impl WatchHandle for NoopWatchHandle {}
