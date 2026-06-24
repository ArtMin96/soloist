//! Adapts coordination's todo store to the supervisor's [`LockReleaser`] port.
//!
//! Sibling to [`LeaseReleaser`](super::LeaseReleaser): the supervisor releases a closing process's
//! locks through the [`LockReleaser`] port without depending on coordination, and this adapter routes
//! that to dropping the process's todo locks over the same [`TodoRepo`] the [`Todos`](super::Todos)
//! aggregate uses. The composition root fans the one close hook out to both releasers (leases and
//! todos) via a [`CompositeLockReleaser`](crate::ports::CompositeLockReleaser), so "locks auto-release
//! when the owning process closes" holds for both with the dependency still pointing one way
//! (supervisor → port ← coordination).

use std::sync::Arc;

use super::todo_repo::TodoRepo;
use crate::ids::ProcessId;
use crate::ports::LockReleaser;

/// A [`LockReleaser`] that drops every todo lock held by a closing process.
pub struct TodoLockReleaser {
    repo: Arc<dyn TodoRepo>,
}

impl TodoLockReleaser {
    /// Over the durable todo store — the same one the aggregate holds, so a release is seen by every
    /// reader.
    pub fn new(repo: Arc<dyn TodoRepo>) -> Self {
        Self { repo }
    }
}

impl LockReleaser for TodoLockReleaser {
    fn release_all(&self, process: ProcessId) {
        // Best-effort, as the port requires: a closing process must never fail because a durable
        // write did. A lock left behind by a failed release is dropped at the next launch's reconcile.
        let _ = self.repo.release_owner(process);
    }
}
