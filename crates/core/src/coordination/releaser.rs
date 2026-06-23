//! Adapts coordination's lease store to the supervisor's [`LockReleaser`] port.
//!
//! The supervisor (process supervision) releases a closing process's locks without depending on
//! coordination: it calls the [`LockReleaser`] port, and this adapter — constructed in the
//! composition root over the same [`LockRepo`] the [`Leases`](super::Leases) aggregate uses —
//! routes that to dropping the process's leases. So "locks auto-release when the owning process
//! closes" holds with the dependency still pointing one way (supervisor → port ← coordination).

use std::sync::Arc;

use super::repo::LockRepo;
use crate::ids::ProcessId;
use crate::ports::LockReleaser;

/// A [`LockReleaser`] that drops every lease owned by a closing process.
pub struct LeaseReleaser {
    repo: Arc<dyn LockRepo>,
}

impl LeaseReleaser {
    /// Over the durable lease store — the same one the aggregate holds, so a release is seen by
    /// every reader.
    pub fn new(repo: Arc<dyn LockRepo>) -> Self {
        Self { repo }
    }
}

impl LockReleaser for LeaseReleaser {
    fn release_all(&self, process: ProcessId) {
        // Best-effort, as the port requires: a closing process must never fail because a durable
        // write did. A lease left behind by a failed release is dropped at the next launch's
        // reconcile, so nothing is stranded permanently.
        let _ = self.repo.release_owner(process);
    }
}
