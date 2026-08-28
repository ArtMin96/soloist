//! The file-watch domain's own driven port: a filesystem watcher that reports the changes
//! which may restart a command or reload a project's config. The adapter (`crates/sys`,
//! over `notify`) watches the OS; the core never touches the filesystem here.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::watch::WatchError;

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

    /// Opens one [`WatchSession`] that every directory registered through it shares — the
    /// backend allocates a single watcher instance rather than one per registration, which is
    /// what lets many projects' watches live behind one OS handle. Changes stream to `changes`
    /// until the session is dropped, which stops every watch it holds; a change that could not be
    /// sent (the channel is full or closed) increments `dropped` instead of blocking the
    /// backend's own delivery thread, so a caller can notice the loss and re-plan.
    fn open(
        &self,
        changes: mpsc::Sender<FileChange>,
        dropped: Arc<AtomicU64>,
    ) -> Result<Arc<dyn WatchSession>, WatchError>;

    /// The most watches the backend will grant, when it will say — on Linux,
    /// `fs.inotify.max_user_watches`. `None` when the backend cannot say, so the caller assumes a
    /// conservative default rather than an unbounded one.
    fn capacity(&self) -> Option<usize>;
}

/// A live filesystem watch. Dropping it stops the watch and frees its OS resources, so the
/// reactor holds one per watched root for exactly as long as it watches that root.
pub trait WatchHandle: Send + Sync {}

/// What changed about one path a [`WatchSession`] is watching.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileChangeKind {
    /// Created, or moved in from elsewhere. The backend does not say which — a moved-in
    /// directory and a moved-in file arrive the same way — so a caller that cares has to stat the
    /// path itself.
    Appeared,
    /// Modified in place: its contents or metadata changed, or it was renamed without leaving the
    /// watched tree.
    Modified,
    /// Removed, or moved out of the watched tree.
    Vanished,
}

/// One path that changed under a [`WatchSession`], and how.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileChange {
    /// The absolute path that changed.
    pub path: PathBuf,
    pub kind: FileChangeKind,
}

/// A live set of filesystem watches sharing one backend instance, from [`FileWatcher::open`].
/// Registering every directory through the same session — rather than one backend instance per
/// directory, as [`FileWatcher::watch`]/[`FileWatcher::watch_dir`] do — is what keeps the app to
/// one OS watcher regardless of how many directories, or how many projects, it watches. Dropping
/// the session stops every watch it holds.
///
/// `&self` rather than `&mut self`: registration is unbounded work for a large directory tree and
/// so runs on the blocking pool (`crate::supervision::run_blocking`), which needs `'static +
/// Send` — an implementation that needs mutable backend state guards it itself (a `Mutex`).
pub trait WatchSession: Send + Sync {
    /// Watches `dir` non-recursively: only its direct children are reported.
    fn watch_dir(&self, dir: &Path) -> Result<(), WatchError>;

    /// Watches `root` and everything beneath it. For a small, bounded tree only — the caller
    /// keeps whatever it registers this way inside its own watch budget; the session enforces
    /// none.
    fn watch_tree(&self, root: &Path) -> Result<(), WatchError>;

    /// Stops watching `path`, best-effort: a path already unregistered, or never registered, is
    /// not an error — the backend itself has nothing to say about a watch that is not there.
    fn unwatch(&self, path: &Path);
}

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

    fn open(
        &self,
        _changes: mpsc::Sender<FileChange>,
        _dropped: Arc<AtomicU64>,
    ) -> Result<Arc<dyn WatchSession>, WatchError> {
        Ok(Arc::new(NoopWatchSession))
    }

    fn capacity(&self) -> Option<usize> {
        None
    }
}

/// The [`WatchHandle`] for a no-op watch — its drop stops nothing.
#[derive(Clone, Copy, Default)]
pub struct NoopWatchHandle;

impl WatchHandle for NoopWatchHandle {}

/// The [`WatchSession`] for a no-op watch — every registration succeeds and reports nothing.
#[derive(Clone, Copy, Default)]
pub struct NoopWatchSession;

impl WatchSession for NoopWatchSession {
    fn watch_dir(&self, _dir: &Path) -> Result<(), WatchError> {
        Ok(())
    }

    fn watch_tree(&self, _root: &Path) -> Result<(), WatchError> {
        Ok(())
    }

    fn unwatch(&self, _path: &Path) {}
}
