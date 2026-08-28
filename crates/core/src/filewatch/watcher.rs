//! The file-watch domain's own driven port: a filesystem watcher that reports the changes
//! which may restart a command or reload a project's config. The adapter (`crates/sys`,
//! over `notify`) watches the OS; the core never touches the filesystem here.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::watch::WatchError;

/// Watches project directories for the filesystem changes that drive file-watch restarts,
/// `solo.yml` reloads, and the git status rail.
///
/// A single [`open`](Self::open) call yields one [`WatchSession`] that every directory the app
/// wants watched registers through — one backend instance for the whole app, however many
/// directories or projects it watches. All matching, debouncing, restarting, and reloading is
/// the consuming code's ([`crate::watchset::ProjectWatchSet`] plans and maintains the
/// registrations; [`super::WatchReactor`], [`crate::projects::ConfigWatchReactor`], and
/// [`crate::git::GitStatusWatchReactor`] consume its fan-out) — the adapter only reports raw
/// changes, so every testable decision stays in the core.
pub trait FileWatcher: Send + Sync {
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

/// A live set of filesystem watches sharing one backend instance, from [`FileWatcher::open`] —
/// what keeps the app to one OS watcher regardless of how many directories, or how many
/// projects, it watches. Dropping the session stops every watch it holds.
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
