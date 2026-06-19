//! A [`LockReleaser`] fake that records which processes it was asked to release locks
//! for, so a test can assert the supervisor frees a process's locks when it closes.

use std::sync::{Arc, Mutex};

use crate::ids::ProcessId;
use crate::ports::LockReleaser;
use crate::sync::lock;

/// A [`LockReleaser`] that records which processes it was asked to release locks for,
/// so a test can assert the supervisor frees a process's locks when it closes.
#[derive(Clone, Default)]
pub struct RecordingLockReleaser {
    released: Arc<Mutex<Vec<ProcessId>>>,
}

impl RecordingLockReleaser {
    pub fn new() -> Self {
        Self::default()
    }

    /// The processes whose locks have been released, in order.
    pub fn released(&self) -> Vec<ProcessId> {
        lock(&self.released).clone()
    }
}

impl LockReleaser for RecordingLockReleaser {
    fn release_all(&self, process: ProcessId) {
        lock(&self.released).push(process);
    }
}
