//! The durable store of coordination leases and the port over it (context C6).
//!
//! This port is dumb persistence keyed by `(project, key)`; the expiry *policy* (comparing a
//! stored deadline against the wall clock) lives in the [`Leases`](super::Leases) aggregate, so
//! the store stays a faithful, time-agnostic record. The bounded context owns its own port (with
//! a [`NoopLockRepo`] default) rather than the shared ports module, so coordination persistence
//! stays confined to coordination.

use crate::ids::{ProcessId, ProjectId};
use crate::ports::StoreError;

/// A persisted lease: a named, project-scoped signal a process holds until it releases it, the
/// process closes, or the absolute expiry passes. Times are Unix milliseconds (a persistable
/// wall clock, [`Clock::now_unix_millis`](crate::ports::Clock::now_unix_millis)) so a deadline
/// written on one run is comparable when read on a later one. The owner is a [`ProcessId`], which
/// is per-run: a lease left by a previous run names a process that no longer exists, so launch
/// reconciliation drops it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredLease {
    pub project: ProjectId,
    pub key: String,
    pub owner: ProcessId,
    pub acquired_unix_millis: u64,
    pub expires_unix_millis: u64,
}

/// Durable repository of coordination leases. One row per `(project, key)`; the aggregate applies
/// the TTL policy on read, so [`get`](LockRepo::get) returns whatever is stored regardless of
/// expiry. Synchronous like the other repositories: the backing reads/writes are tiny and local.
pub trait LockRepo: Send + Sync {
    /// The lease stored for `(project, key)`, or `None` — without applying expiry (the caller's
    /// aggregate decides whether it is still live).
    fn get(&self, project: ProjectId, key: &str) -> Result<Option<StoredLease>, StoreError>;
    /// Inserts or replaces the lease for its `(project, key)`.
    fn put(&self, lease: &StoredLease) -> Result<(), StoreError>;
    /// Removes the lease for `(project, key)`; returns whether one was present.
    fn remove(&self, project: ProjectId, key: &str) -> Result<bool, StoreError>;
    /// Removes every lease owned by `owner` — the process closed — returning how many. Called
    /// from the supervisor's lock-release hook, so it must stay cheap and never block.
    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError>;
    /// Clears every lease — launch reconciliation. Leases are process-owned, and a per-run
    /// [`ProcessId`] is recycled across runs (the counter restarts each launch), so a lease left
    /// by a previous run can never be matched safely to this run's processes. No process from a
    /// fresh run holds a lease yet, so clearing the table is the correct, safe reconcile. Returns
    /// how many were cleared.
    fn clear(&self) -> Result<usize, StoreError>;
}

/// A [`LockRepo`] that stores nothing — the default until the durable adapter is wired, so the
/// core runs (coordination simply persists nothing) without it. Acquiring always reads free.
#[derive(Clone, Copy, Default)]
pub struct NoopLockRepo;

impl LockRepo for NoopLockRepo {
    fn get(&self, _project: ProjectId, _key: &str) -> Result<Option<StoredLease>, StoreError> {
        Ok(None)
    }
    fn put(&self, _lease: &StoredLease) -> Result<(), StoreError> {
        Ok(())
    }
    fn remove(&self, _project: ProjectId, _key: &str) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn release_owner(&self, _owner: ProcessId) -> Result<usize, StoreError> {
        Ok(0)
    }
    fn clear(&self) -> Result<usize, StoreError> {
        Ok(0)
    }
}
