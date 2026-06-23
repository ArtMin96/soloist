//! The durable store of coordination leases and the port over it (context C6).
//!
//! The port is deliberately small and **atomic**: each operation that depends on the current
//! state — acquiring, releasing, reading-with-prune — is one method the adapter performs
//! indivisibly, never a separate read the aggregate then acts on. That is what makes a lease a
//! real mutual-exclusion signal under concurrent callers (each MCP client is its own connection,
//! so two agents in one project can acquire the same key at once). The expiry *rule* a lease is
//! live iff its `expires_unix_millis` is **strictly after** `now` is part of this contract; the
//! TTL *policy* (default, clamp) lives in the [`Leases`](super::Leases) aggregate. The bounded
//! context owns its own port (with a [`NoopLockRepo`] default) rather than the shared ports
//! module, so coordination persistence stays confined to coordination.

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

/// Durable repository of coordination leases. One row per `(project, key)`. Every method is
/// atomic with respect to the others, so a check-and-write (acquire, owner-scoped release) cannot
/// interleave with a concurrent caller and grant the same key twice.
pub trait LockRepo: Send + Sync {
    /// Atomically acquires `candidate`'s `(project, key)` for its owner, treating a lease whose
    /// `expires_unix_millis` is at or before `now` as free. Grants — writing `candidate` — when
    /// the slot is free, expired, or already held by `candidate.owner` (a renewal), and returns
    /// `Ok(None)`. Otherwise it writes nothing and returns `Ok(Some(holder))` with the live
    /// holder that blocked it, so the caller can report contention without the call ever blocking.
    fn acquire(&self, candidate: &StoredLease, now: u64)
        -> Result<Option<StoredLease>, StoreError>;

    /// The live holder of `(project, key)` at `now`, or `None` if free or expired. Prunes the row
    /// when it has expired — the prune is conditional on expiry, so it never removes a lease a
    /// concurrent caller just acquired.
    fn live(
        &self,
        project: ProjectId,
        key: &str,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError>;

    /// Releases `(project, key)` only if `owner` holds it, returning whether a lease was removed.
    /// The ownership check is part of the removal, so a release can never drop another owner's
    /// lease — including one a concurrent caller acquired between.
    fn release(&self, project: ProjectId, key: &str, owner: ProcessId) -> Result<bool, StoreError>;

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
/// core runs (coordination simply persists nothing) without it. Acquiring always grants but
/// nothing is retained, so a later read reports the key free.
#[derive(Clone, Copy, Default)]
pub struct NoopLockRepo;

impl LockRepo for NoopLockRepo {
    fn acquire(
        &self,
        _candidate: &StoredLease,
        _now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        Ok(None)
    }
    fn live(
        &self,
        _project: ProjectId,
        _key: &str,
        _now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        Ok(None)
    }
    fn release(
        &self,
        _project: ProjectId,
        _key: &str,
        _owner: ProcessId,
    ) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn release_owner(&self, _owner: ProcessId) -> Result<usize, StoreError> {
        Ok(0)
    }
    fn clear(&self) -> Result<usize, StoreError> {
        Ok(0)
    }
}
