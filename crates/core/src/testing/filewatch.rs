//! A [`FileWatcher`] fake for the watch reactors' tests: it holds the roots the reactor asked it
//! to watch and delivers a test's synthetic changed paths to the ones that cover them, without
//! touching the OS.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, Notify};

use crate::filewatch::{FileChange, FileChangeKind, FileWatcher, WatchHandle, WatchSession};
use crate::sync::lock;
use crate::testing::wait::bounded;
use crate::watch::WatchError;

/// An in-memory [`FileWatcher`] that records the roots it was asked to watch and delivers
/// [`FakeFileWatcher::change`] to the live watches covering the changed path — recursively for
/// [`FileWatcher::watch`], direct children only for [`FileWatcher::watch_dir`], as the real
/// adapter does. Routing by root rather than to whichever sink came last is what makes *which*
/// roots a reactor watches observable: a reactor that stops watching a tree stops hearing about it.
///
/// Each handle it returns removes its watch from the live set on drop, so a test can also assert a
/// watch was **released**. [`FakeFileWatcher::refuse`] makes a root unwatchable, which is how a
/// test states that the OS turned a watch down.
#[derive(Default)]
pub struct FakeFileWatcher {
    refused: Arc<Mutex<Vec<PathBuf>>>,
    watches: Arc<Mutex<Watches>>,
    established: Notify,
    released: Arc<Notify>,
    sessions: Arc<Mutex<Sessions>>,
    capacity: Option<usize>,
    open_refused: Mutex<bool>,
    /// The counter [`FileWatcher::open`]'s most recent caller passed, so [`Self::change_of`] can
    /// mirror the real adapter's contract: a change that could not be delivered still bumps it.
    dropped: Mutex<Option<Arc<AtomicU64>>>,
}

/// What the fake knows about the watches it was asked for, under one lock so a test that sees a
/// root recorded also sees the watch (or the refusal) that answered it.
#[derive(Default)]
struct Watches {
    /// Every root asked for, in order, granted or not.
    requested: Vec<PathBuf>,
    live: Vec<Live>,
    next: usize,
}

/// One live watch: which paths it covers, and where their changes go.
struct Live {
    id: usize,
    root: PathBuf,
    recursive: bool,
    sink: mpsc::Sender<PathBuf>,
}

impl Live {
    /// Whether a change to `path` is one this watch reports: anywhere beneath its root when it is
    /// recursive, and a direct child of it when it is not.
    fn covers(&self, path: &Path) -> bool {
        if self.recursive {
            path.starts_with(&self.root)
        } else {
            path.parent() == Some(self.root.as_path())
        }
    }
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
    /// [`WatchSession::watch_dir`] — the same rule [`Live::covers`] applies for the legacy port.
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
    /// does. A later [`FileWatcher::watch`] or [`FileWatcher::watch_dir`] call for it succeeds; a
    /// watch already refused before this call is not retroactively granted — the caller has to ask
    /// again.
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

    /// Feeds a synthetic changed absolute path to every live watch covering it (best-effort, like
    /// the real adapter). A path no watch covers reaches the reactor no more than a change in an
    /// unwatched tree would.
    pub fn change(&self, path: impl Into<PathBuf>) {
        let path = path.into();
        for watch in lock(&self.watches)
            .live
            .iter()
            .filter(|live| live.covers(&path))
        {
            let _ = watch.sink.try_send(path.clone());
        }
    }

    /// The roots the reactor asked to watch — lets a test assert that an ineligible command
    /// is never watched, and that a root it was refused was still asked for.
    pub fn watched(&self) -> Vec<PathBuf> {
        lock(&self.watches).requested.clone()
    }

    /// The roots whose watch handle is still alive — the watches currently holding OS
    /// resources. Unlike [`Self::watched`] (a log of every request), this shrinks when the
    /// reactor drops a handle, so a test can assert a watch was released.
    pub fn live(&self) -> Vec<PathBuf> {
        lock(&self.watches)
            .live
            .iter()
            .map(|live| live.root.clone())
            .collect()
    }

    /// Resolves once the reactor has registered at least one watch — a deterministic signal
    /// to await instead of polling [`watched`], since [`FileWatcher::watch`] notifies here.
    /// A watch registered before this is awaited is not missed (the notification is retained).
    ///
    /// [`watched`]: Self::watched
    pub async fn established(&self) {
        bounded(
            "the reactor to register a watch",
            self.established.notified(),
        )
        .await;
    }

    /// Resolves once the reactor has asked to watch `root` — granted or refused. What a test needs
    /// when the reactor registers several watches and the one under test is not the first:
    /// [`Self::established`] fires on whichever arrived, which says nothing about the rest.
    pub async fn asked_for(&self, root: &Path) {
        bounded(&format!("a watch request for {}", root.display()), async {
            while !lock(&self.watches)
                .requested
                .iter()
                .any(|asked| asked == root)
            {
                self.established.notified().await;
            }
        })
        .await;
    }

    /// Resolves once the reactor has dropped at least one watch handle — the deterministic
    /// mirror of [`Self::established`] for asserting a watch was released.
    pub async fn released(&self) {
        bounded("the reactor to drop a watch", self.released.notified()).await;
    }

    /// Sets what [`FileWatcher::capacity`] reports, backing a test's `Budget` scenarios. Consumed
    /// at construction — before the fake is shared behind an `Arc` — because a capacity a test
    /// changes mid-run would race whatever already read it.
    pub fn with_capacity(mut self, watches: usize) -> Self {
        self.capacity = Some(watches);
        self
    }

    /// The paths registered through [`FileWatcher::open`]'s session and still held by it — a
    /// dropped session's registrations disappear from here together, mirroring [`Self::live`]'s
    /// semantics for the session-based port.
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
    /// (best-effort, like [`Self::change`] for the legacy port). A send that could not be
    /// delivered bumps the `dropped` counter [`FileWatcher::open`]'s caller was given, mirroring
    /// the real adapter's contract instead of silently discarding it.
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

    fn record(
        &self,
        root: PathBuf,
        recursive: bool,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        let refused = lock(&self.refused).contains(&root);
        let established = {
            let mut watches = lock(&self.watches);
            watches.requested.push(root.clone());
            if refused {
                None
            } else {
                let id = watches.next;
                watches.next += 1;
                watches.live.push(Live {
                    id,
                    root,
                    recursive,
                    sink: changes,
                });
                Some(id)
            }
        };
        self.established.notify_one();
        match established {
            Some(id) => Ok(Box::new(FakeWatchHandle {
                id,
                watches: self.watches.clone(),
                released: self.released.clone(),
            })),
            None => Err(WatchError::BudgetExhausted),
        }
    }
}

impl FileWatcher for FakeFileWatcher {
    fn watch(
        &self,
        root: PathBuf,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        self.record(root, true, changes)
    }

    fn watch_dir(
        &self,
        dir: PathBuf,
        changes: mpsc::Sender<PathBuf>,
    ) -> Result<Box<dyn WatchHandle>, WatchError> {
        self.record(dir, false, changes)
    }

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

/// The live-set bookkeeping handle [`FakeFileWatcher`] returns: dropping it removes its watch
/// from the live set and signals [`FakeFileWatcher::released`], mimicking a real watch
/// releasing its OS resources.
struct FakeWatchHandle {
    id: usize,
    watches: Arc<Mutex<Watches>>,
    released: Arc<Notify>,
}

impl WatchHandle for FakeWatchHandle {}

impl Drop for FakeWatchHandle {
    fn drop(&mut self) {
        let mut watches = lock(&self.watches);
        if let Some(at) = watches.live.iter().position(|watch| watch.id == self.id) {
            watches.live.remove(at);
        }
        drop(watches);
        self.released.notify_one();
    }
}

/// The [`WatchSession`] [`FakeFileWatcher::open`] returns: registrations made through it honour
/// the same [`FakeFileWatcher::refuse`]/[`allow`](FakeFileWatcher::allow) list a legacy
/// [`FileWatcher::watch`]/[`FileWatcher::watch_dir`] call would, and dropping it releases every
/// path it registered together — the same OS-resource-release contract [`FakeWatchHandle`] gives
/// the legacy port, at session granularity instead of per-path.
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
