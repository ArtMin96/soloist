//! A [`FileWatcher`] fake for the watch reactor's tests: it captures the change sink the
//! reactor hands it and lets a test feed synthetic changed paths, without touching the OS.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, Notify};

use crate::filewatch::{FileWatcher, WatchHandle};
use crate::sync::lock;

/// An in-memory [`FileWatcher`] that records the roots it was asked to watch and the change
/// sink, so a test can drive [`FakeFileWatcher::change`] to simulate a filesystem event. Every
/// sink the reactor passes is a clone of its one channel, so capturing the most recent is
/// enough to reach the reactor for any watched root. Each handle it returns removes its root
/// from the live set on drop, so a test can also assert a watch was **released**.
#[derive(Default)]
pub struct FakeFileWatcher {
    roots: Mutex<Vec<PathBuf>>,
    live: Arc<Mutex<Vec<PathBuf>>>,
    sink: Mutex<Option<mpsc::Sender<PathBuf>>>,
    established: Notify,
    released: Arc<Notify>,
}

impl FakeFileWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a synthetic changed absolute path to the reactor (best-effort, like the real
    /// adapter). A no-op before the reactor has registered a watch.
    pub fn change(&self, path: impl Into<PathBuf>) {
        if let Some(sink) = lock(&self.sink).as_ref() {
            let _ = sink.try_send(path.into());
        }
    }

    /// The roots the reactor asked to watch — lets a test assert that an ineligible command
    /// is never watched.
    pub fn watched(&self) -> Vec<PathBuf> {
        lock(&self.roots).clone()
    }

    /// The roots whose watch handle is still alive — the watches currently holding OS
    /// resources. Unlike [`Self::watched`] (a log of every request), this shrinks when the
    /// reactor drops a handle, so a test can assert a watch was released.
    pub fn live(&self) -> Vec<PathBuf> {
        lock(&self.live).clone()
    }

    /// Resolves once the reactor has registered at least one watch — a deterministic signal
    /// to await instead of polling [`watched`], since [`FileWatcher::watch`] notifies here.
    /// A watch registered before this is awaited is not missed (the notification is retained).
    ///
    /// [`watched`]: Self::watched
    pub async fn established(&self) {
        self.established.notified().await;
    }

    /// Resolves once the reactor has dropped at least one watch handle — the deterministic
    /// mirror of [`Self::established`] for asserting a watch was released.
    pub async fn released(&self) {
        self.released.notified().await;
    }

    fn record(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        lock(&self.roots).push(root.clone());
        lock(&self.live).push(root.clone());
        *lock(&self.sink) = Some(changes);
        self.established.notify_one();
        Box::new(FakeWatchHandle {
            root,
            live: self.live.clone(),
            released: self.released.clone(),
        })
    }
}

impl FileWatcher for FakeFileWatcher {
    fn watch(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        self.record(root, changes)
    }

    // Recursion depth is the real adapter's concern; the fake records both the same way, so
    // a reactor's watch bookkeeping is asserted identically whichever method it drives.
    fn watch_dir(&self, dir: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        self.record(dir, changes)
    }
}

/// The live-set bookkeeping handle [`FakeFileWatcher`] returns: dropping it removes its root
/// from the live set and signals [`FakeFileWatcher::released`], mimicking a real watch
/// releasing its OS resources.
struct FakeWatchHandle {
    root: PathBuf,
    live: Arc<Mutex<Vec<PathBuf>>>,
    released: Arc<Notify>,
}

impl WatchHandle for FakeWatchHandle {}

impl Drop for FakeWatchHandle {
    fn drop(&mut self) {
        let mut live = lock(&self.live);
        if let Some(at) = live.iter().position(|root| root == &self.root) {
            live.remove(at);
        }
        drop(live);
        self.released.notify_one();
    }
}
