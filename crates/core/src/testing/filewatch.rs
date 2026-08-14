//! A [`FileWatcher`] fake for the watch reactors' tests: it holds the roots the reactor asked it
//! to watch and delivers a test's synthetic changed paths to the ones that cover them, without
//! touching the OS.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, Notify};

use crate::filewatch::{FileWatcher, WatchHandle};
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
    refused: Mutex<Vec<PathBuf>>,
    watches: Arc<Mutex<Watches>>,
    established: Notify,
    released: Arc<Notify>,
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
