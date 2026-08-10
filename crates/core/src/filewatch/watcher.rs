//! The file-watch domain's own driven port: a filesystem watcher that reports the changes
//! which may restart a command or reload a project's config. The adapter (`crates/sys`,
//! over `notify`) watches the OS; the core never touches the filesystem here.

use std::path::PathBuf;

use thiserror::Error;
use tokio::sync::mpsc;

/// Why a directory could not be watched.
///
/// A closed set, because the core's answer differs by case: an exhausted budget is the user's to
/// raise and is worth saying so about, while a path that is not there may simply have gone. A watch
/// that cannot be established is reported rather than swallowed — a watch that silently yields no
/// events is indistinguishable from a tree nothing ever changes in, which is the one failure nobody
/// notices.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
pub enum WatchError {
    /// The OS refused another watch because the per-user file-watch budget is exhausted. On Linux
    /// that is `fs.inotify.max_user_watches`, shared with every other program on the machine, and a
    /// recursive watch spends one per directory beneath its root.
    #[error("the system's file-watch limit is exhausted")]
    BudgetExhausted,
    /// The path itself could not be watched: it does not exist, is not readable, or vanished while
    /// the watch was being established.
    #[error("the directory could not be watched")]
    Unwatchable,
    /// The watching backend could not be started at all, so nothing under this root will report.
    #[error("the filesystem watcher is unavailable")]
    Unavailable,
}

/// Watches project directories for the filesystem changes that drive file-watch restarts
/// and `solo.yml` reloads.
///
/// An implementation watches a directory for the events that leave it different than before —
/// a path created, modified (a rename included), or removed — and forwards each changed
/// **absolute** path to the `changes` channel. A path merely opened or read is not a change and
/// is not reported. All matching, debouncing, restarting, and reloading is the consuming
/// reactor's ([`super::WatchReactor`], [`crate::projects::ConfigWatchReactor`],
/// [`crate::git::GitStatusWatchReactor`]) — the adapter only reports raw changes, so every
/// testable decision stays in the core.
///
/// Both methods block until the watch is registered. [`Self::watch`] walks the whole tree under its
/// root to do it, which is unbounded work, so a caller reaches it on the blocking pool rather than on
/// a runtime worker; [`Self::watch_dir`] registers one directory and is called directly.
pub trait FileWatcher: Send + Sync {
    /// Begins watching `root` recursively, forwarding each changed absolute path to `changes`
    /// until the returned [`WatchHandle`] is dropped (which stops the watch and releases its
    /// OS resources — the bounded-resource contract).
    ///
    /// A root that cannot be watched is a [`WatchError`], never a handle that reports nothing: the
    /// caller decides how to degrade, and says so.
    fn watch(
        &self,
        root: PathBuf,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError>;

    /// Begins watching the single directory `dir` — **non-recursive**, so only its direct
    /// children report — with the same channel, handle, and failure contract as [`Self::watch`].
    /// For a file at a fixed, known location (a project root's `solo.yml`), where a recursive tree
    /// watch would spend an OS watch per subdirectory to observe one file.
    fn watch_dir(
        &self,
        dir: PathBuf,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError>;
}

/// A live filesystem watch. Dropping it stops the watch and frees its OS resources, so the
/// reactor holds one per watched root for exactly as long as it watches that root.
pub trait WatchHandle: Send + Sync {}

/// A [`FileWatcher`] that watches nothing — the default until the OS adapter is wired
/// (headless tools, tests that do not exercise watching). The reactor then never restarts.
///
/// It succeeds rather than failing, deliberately: watching nothing here is a composition choice, and
/// a caller that reported it as a failure would warn about every root in a build that never meant to
/// watch one.
#[derive(Clone, Copy, Default)]
pub struct NoopFileWatcher;

impl FileWatcher for NoopFileWatcher {
    fn watch(
        &self,
        _root: PathBuf,
        _changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        Ok(Box::new(NoopWatchHandle))
    }

    fn watch_dir(
        &self,
        _dir: PathBuf,
        _changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        Ok(Box::new(NoopWatchHandle))
    }
}

/// The [`WatchHandle`] for a no-op watch — its drop stops nothing.
#[derive(Clone, Copy, Default)]
pub struct NoopWatchHandle;

impl WatchHandle for NoopWatchHandle {}
