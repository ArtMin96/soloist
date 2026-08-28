//! A [`FileWatcher`] fake for the watch set's tests: it records what was registered through the
//! session [`FileWatcher::open`] returns and delivers a test's synthetic changes to whichever
//! registrations cover them, without touching the OS.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::filewatch::{FileChange, FileChangeKind, FileWatcher, WatchSession};
use crate::sync::lock;
use crate::watch::WatchError;

/// An in-memory [`FileWatcher`] whose [`FileWatcher::open`] hands out a [`FakeWatchSession`]
/// recording every path registered through it and delivering
/// [`FakeFileWatcher::change_of`] to the ones covering the changed path, without touching the OS.
/// [`FakeFileWatcher::refuse`] makes a path unregistrable, which is how a test states that the OS
/// turned a registration down.
#[derive(Default)]
pub struct FakeFileWatcher {
    refused: Arc<Mutex<Vec<PathBuf>>>,
    sessions: Arc<Mutex<Sessions>>,
    capacity: Option<usize>,
    open_refused: Mutex<bool>,
    /// The counter [`FileWatcher::open`]'s most recent caller passed, so [`Self::change_of`] can
    /// mirror the real adapter's contract: a change that could not be delivered still bumps it.
    dropped: Mutex<Option<Arc<AtomicU64>>>,
}

/// What the fake knows about every [`FakeWatchSession`] it has opened, under one lock so a
/// registration recorded there is never seen without the session (or its absence) that made it.
#[derive(Default)]
struct Sessions {
    /// Sessions opened so far — also the next one's generation id.
    opened: usize,
    /// Every path registered by a still-live session, generation-tagged so the registrations of a
    /// dropped session disappear from here together, as the real backend's OS resources would.
    live: Vec<SessionEntry>,
    /// Every path any session has asked to stop watching, in order.
    unwatched: Vec<PathBuf>,
}

/// One path a [`FakeWatchSession`] holds: where its changes go, so [`FakeFileWatcher::change_of`]
/// can deliver without going back through the session that registered it.
struct SessionEntry {
    generation: usize,
    path: PathBuf,
    recursive: bool,
    sink: mpsc::Sender<FileChange>,
}

impl SessionEntry {
    /// Whether a change to `path` is one this registration reports: anywhere beneath its path
    /// when it came from [`WatchSession::watch_tree`], a direct child of it when it came from
    /// [`WatchSession::watch_dir`].
    fn covers(&self, path: &Path) -> bool {
        if self.recursive {
            path.starts_with(&self.path)
        } else {
            path.parent() == Some(self.path.as_path())
        }
    }
}

impl FakeFileWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Refuses `root` from here on, as an OS out of watch descriptors does. The request is still
    /// recorded (the reactor asked for it), but nothing is watched there and no change is delivered.
    ///
    /// Set after construction, because the root a test refuses is usually one the project registry
    /// chose for it.
    pub fn refuse(&self, root: impl Into<PathBuf>) {
        lock(&self.refused).push(root.into());
    }

    /// Reverses [`Self::refuse`]: `root` can be watched again, as a budget that has since freed up
    /// does. A later registration for it succeeds; a watch already refused before this call is
    /// not retroactively granted — the caller has to ask again.
    pub fn allow(&self, root: impl Into<PathBuf>) {
        let root = root.into();
        lock(&self.refused).retain(|refused| *refused != root);
    }

    /// Fails every [`FileWatcher::open`] call from here on, as a backend that could not start
    /// does. Unlike [`Self::refuse`], which turns down one root a session would otherwise
    /// register, this turns down the session itself.
    pub fn refuse_open(&self) {
        *lock(&self.open_refused) = true;
    }

    /// Reverses [`Self::refuse_open`]: the next [`FileWatcher::open`] call succeeds again.
    pub fn allow_open(&self) {
        *lock(&self.open_refused) = false;
    }

    /// Sets what [`FileWatcher::capacity`] reports, backing a test's `Budget` scenarios. Consumed
    /// at construction — before the fake is shared behind an `Arc` — because a capacity a test
    /// changes mid-run would race whatever already read it.
    pub fn with_capacity(mut self, watches: usize) -> Self {
        self.capacity = Some(watches);
        self
    }

    /// The paths registered through [`FileWatcher::open`]'s session and still held by it — a
    /// dropped session's registrations disappear from here together.
    pub fn registered(&self) -> Vec<PathBuf> {
        lock(&self.sessions)
            .live
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    /// Every path a session has asked [`WatchSession::unwatch`] to stop watching, in order — the
    /// log a test checks a released registration against.
    pub fn unwatched(&self) -> Vec<PathBuf> {
        lock(&self.sessions).unwatched.clone()
    }

    /// How many sessions [`FileWatcher::open`] has handed out — the resource-lifecycle fact that
    /// tells a rebuilt session (a fresh backend instance) apart from a reused one, which
    /// [`Self::registered`] alone cannot: both look identical if the registrations were simply
    /// replayed onto the same session.
    pub fn sessions_opened(&self) -> usize {
        lock(&self.sessions).opened
    }

    /// Feeds a synthetic [`FileChange`] to every live session registration covering `path`
    /// (best-effort, like the real adapter). A send that could not be delivered bumps the
    /// `dropped` counter [`FileWatcher::open`]'s caller was given, mirroring the real adapter's
    /// contract instead of silently discarding it.
    pub fn change_of(&self, path: impl Into<PathBuf>, kind: FileChangeKind) {
        let path = path.into();
        let dropped = lock(&self.dropped).clone();
        for entry in lock(&self.sessions)
            .live
            .iter()
            .filter(|entry| entry.covers(&path))
        {
            let sent = entry.sink.try_send(FileChange {
                path: path.clone(),
                kind,
            });
            if sent.is_err() {
                if let Some(dropped) = &dropped {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

impl FileWatcher for FakeFileWatcher {
    fn open(
        &self,
        changes: mpsc::Sender<FileChange>,
        dropped: Arc<AtomicU64>,
    ) -> Result<Arc<dyn WatchSession>, WatchError> {
        if *lock(&self.open_refused) {
            return Err(WatchError::Unavailable);
        }
        *lock(&self.dropped) = Some(dropped);
        let generation = {
            let mut sessions = lock(&self.sessions);
            let generation = sessions.opened;
            sessions.opened += 1;
            generation
        };
        Ok(Arc::new(FakeWatchSession {
            generation,
            changes,
            refused: self.refused.clone(),
            sessions: self.sessions.clone(),
        }))
    }

    fn capacity(&self) -> Option<usize> {
        self.capacity
    }
}

/// The [`WatchSession`] [`FakeFileWatcher::open`] returns: registrations made through it honour
/// the same [`FakeFileWatcher::refuse`]/[`allow`](FakeFileWatcher::allow) list, and dropping it
/// releases every path it registered together, mimicking a real watch session releasing its OS
/// resources.
struct FakeWatchSession {
    generation: usize,
    changes: mpsc::Sender<FileChange>,
    refused: Arc<Mutex<Vec<PathBuf>>>,
    sessions: Arc<Mutex<Sessions>>,
}

impl FakeWatchSession {
    fn register(&self, path: PathBuf, recursive: bool) -> Result<(), WatchError> {
        if lock(&self.refused).contains(&path) {
            return Err(WatchError::BudgetExhausted);
        }
        lock(&self.sessions).live.push(SessionEntry {
            generation: self.generation,
            path,
            recursive,
            sink: self.changes.clone(),
        });
        Ok(())
    }
}

impl WatchSession for FakeWatchSession {
    fn watch_dir(&self, dir: &Path) -> Result<(), WatchError> {
        self.register(dir.to_path_buf(), false)
    }

    fn watch_tree(&self, root: &Path) -> Result<(), WatchError> {
        self.register(root.to_path_buf(), true)
    }

    fn unwatch(&self, path: &Path) {
        let mut sessions = lock(&self.sessions);
        sessions
            .live
            .retain(|entry| !(entry.generation == self.generation && entry.path == path));
        sessions.unwatched.push(path.to_path_buf());
    }
}

impl Drop for FakeWatchSession {
    fn drop(&mut self) {
        lock(&self.sessions)
            .live
            .retain(|entry| entry.generation != self.generation);
    }
}
