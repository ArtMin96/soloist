//! Session-scoped coordination actions (context C8 → C6): the lease surface a remote caller
//! (MCP today) drives within its effective project.
//!
//! A lease is project-scoped and process-owned, so each method resolves two things in the core —
//! the session's **effective project** (what the lease belongs to) and its **bound process** (who
//! owns it) — before routing to the one [`Leases`](crate::coordination::Leases) aggregate. Both
//! are resolved here, not in any adapter, so every remote surface inherits the identical scope and
//! ownership rules. The bound process must be authentic (it was checked at bind time), which is
//! also what lets the supervisor auto-release the lease when that process closes.

use std::time::Duration;

use super::Facade;
use crate::coordination::{AcquireOutcome, LeaseView};
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::ports::StoreError;

/// Why a coordination action was refused. Mapped by the wire adapters to their own error type, so
/// the taxonomy is defined once here.
#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    /// The session has no project in scope to act within (none selected, bound, or singular).
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The session is not bound to a process, so it has no owner to attribute a lease to. An
    /// agent binds via its injected `SOLOIST_PROCESS_ID`; an unbound external caller cannot hold
    /// a process-owned lease (it has nothing to auto-release it on close).
    #[error("not bound to a process; bind a session before holding a lease")]
    NoBoundProcess,
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Facade {
    /// Acquires the lease `key` in the session's effective project, owned by its bound process,
    /// for `ttl` (clamped by the aggregate). Non-blocking: if the key is already held by another
    /// process, returns [`AcquireOutcome::Held`] with the holder rather than waiting. Re-acquiring
    /// a key the caller already holds renews it.
    pub fn lock_acquire(
        &self,
        session: SessionId,
        key: &str,
        ttl: Duration,
    ) -> Result<AcquireOutcome, CoordinationError> {
        let project = self.lease_scope(session)?;
        let owner = self.lease_owner(session)?;
        Ok(self.leases.acquire(project, key, owner, ttl)?)
    }

    /// The current holder of the lease `key` in the session's effective project, or `None` if it
    /// is free or has expired. A read — it needs the project scope but not a bound process.
    pub fn lock_status(
        &self,
        session: SessionId,
        key: &str,
    ) -> Result<Option<LeaseView>, CoordinationError> {
        let project = self.lease_scope(session)?;
        Ok(self.leases.status(project, key)?)
    }

    /// Releases the lease `key` in the session's effective project if it is held by the caller's
    /// bound process, returning whether the caller's lease was released. A caller cannot release a
    /// lease another process holds.
    pub fn lock_release(&self, session: SessionId, key: &str) -> Result<bool, CoordinationError> {
        let project = self.lease_scope(session)?;
        let owner = self.lease_owner(session)?;
        Ok(self.leases.release(project, key, owner)?)
    }

    /// Clears every stale lease on launch — see [`Leases::reconcile`](crate::coordination::Leases::reconcile).
    /// Not session-scoped; the composition root calls it once at startup.
    pub fn reconcile_leases(&self) -> Result<usize, StoreError> {
        self.leases.reconcile()
    }

    /// The session's effective project, or [`CoordinationError::NoProjectScope`].
    fn lease_scope(&self, session: SessionId) -> Result<ProjectId, CoordinationError> {
        self.effective_project(session)
            .ok_or(CoordinationError::NoProjectScope)
    }

    /// The session's bound process — the lease owner — or [`CoordinationError::NoBoundProcess`].
    fn lease_owner(&self, session: SessionId) -> Result<ProcessId, CoordinationError> {
        self.identity
            .origin(session)
            .process()
            .ok_or(CoordinationError::NoBoundProcess)
    }
}

#[cfg(test)]
#[path = "coordination_tests.rs"]
mod tests;
