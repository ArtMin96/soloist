//! A [`FileWatcher`] fake for the watch reactor's tests: it captures the change sink the
//! reactor hands it and lets a test feed synthetic changed paths, without touching the OS.

use std::path::PathBuf;
use std::sync::Mutex;

use tokio::sync::{mpsc, Notify};

use crate::filewatch::{FileWatcher, NoopWatchHandle, WatchHandle};
use crate::sync::lock;

/// An in-memory [`FileWatcher`] that records the roots it was asked to watch and the change
/// sink, so a test can drive [`FakeFileWatcher::change`] to simulate a filesystem event. Every
/// sink the reactor passes is a clone of its one channel, so capturing the most recent is
/// enough to reach the reactor for any watched root.
#[derive(Default)]
pub struct FakeFileWatcher {
    roots: Mutex<Vec<PathBuf>>,
    sink: Mutex<Option<mpsc::Sender<PathBuf>>>,
    established: Notify,
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

    /// Resolves once the reactor has registered at least one watch — a deterministic signal
    /// to await instead of polling [`watched`], since [`FileWatcher::watch`] notifies here.
    /// A watch registered before this is awaited is not missed (the notification is retained).
    pub async fn established(&self) {
        self.established.notified().await;
    }
}

impl FileWatcher for FakeFileWatcher {
    fn watch(&self, root: PathBuf, changes: mpsc::Sender<PathBuf>) -> Box<dyn WatchHandle> {
        lock(&self.roots).push(root);
        *lock(&self.sink) = Some(changes);
        self.established.notify_one();
        Box::new(NoopWatchHandle)
    }
}
