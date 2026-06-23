//! Session-scoped coordination actions (context C8 → C6): the lease and timer surface a remote
//! caller (MCP today) drives within its effective project.
//!
//! Leases and timers are project-scoped and process-owned, so each method resolves two things in
//! the core — the session's **effective project** (what the record belongs to) and its **bound
//! process** (who owns it) — before routing to the one [`Leases`](crate::coordination::Leases) or
//! [`Timers`](crate::coordination::Timers) aggregate. Both are resolved here, not in any adapter,
//! so every remote surface inherits the identical scope and ownership rules. The bound process must
//! be authentic (it was checked at bind time), which is also what lets the supervisor auto-release a
//! lease — and what a fired timer delivers its body to — when that process closes.

use std::time::Duration;

use super::Facade;
use crate::coordination::{
    watched_is_idle, AcquireOutcome, IdleMode, LeaseView, SetWhenIdleOutcome, TimerView,
};
use crate::ids::{ProcessId, ProjectId, SessionId, TimerId};
use crate::ports::StoreError;

/// Why a coordination action was refused. Mapped by the wire adapters to their own error type, so
/// the taxonomy is defined once here.
#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    /// The session has no project in scope to act within (none selected, bound, or singular).
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The session is not bound to a process, so it has no owner to attribute the record to. An
    /// agent binds via its injected `SOLOIST_PROCESS_ID`; an unbound external caller cannot own a
    /// process-owned coordination record — a lease or a timer (nothing would deliver a timer's body
    /// or auto-release a lease on close).
    #[error("not bound to a process; bind a session before owning a timer or lease")]
    NoBoundProcess,
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Facade {
    /// Acquires the lease `key` in the session's effective project, owned by its bound process,
    /// for `ttl` (the aggregate's default when `None`, bounded by it otherwise). Non-blocking: if
    /// the key is already held by another process, returns [`AcquireOutcome::Held`] with the
    /// holder rather than waiting. Re-acquiring a key the caller already holds renews it.
    pub fn lock_acquire(
        &self,
        session: SessionId,
        key: &str,
        ttl: Option<Duration>,
    ) -> Result<AcquireOutcome, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let owner = self.coordination_owner(session)?;
        Ok(self.leases.acquire(project, key, owner, ttl)?)
    }

    /// The current holder of the lease `key` in the session's effective project, or `None` if it
    /// is free or has expired. A read — it needs the project scope but not a bound process.
    pub fn lock_status(
        &self,
        session: SessionId,
        key: &str,
    ) -> Result<Option<LeaseView>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        Ok(self.leases.status(project, key)?)
    }

    /// Releases the lease `key` in the session's effective project if it is held by the caller's
    /// bound process, returning whether the caller's lease was released. A caller cannot release a
    /// lease another process holds.
    pub fn lock_release(&self, session: SessionId, key: &str) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let owner = self.coordination_owner(session)?;
        Ok(self.leases.release(project, key, owner)?)
    }

    /// Clears every stale lease on launch — see [`Leases::reconcile`](crate::coordination::Leases::reconcile).
    /// Not session-scoped; the composition root calls it once at startup.
    pub fn reconcile_leases(&self) -> Result<usize, StoreError> {
        self.leases.reconcile()
    }

    /// Arms a plain timer in the session's effective project, owned by its bound process, that
    /// delivers `body` to that process as a fresh turn after `after` (immediately when `None`).
    /// Needs a bound process — the owner the body is delivered to and that the timer is cleaned up
    /// with on close.
    pub fn timer_set(
        &self,
        session: SessionId,
        body: String,
        after: Option<Duration>,
    ) -> Result<TimerView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let owner = self.coordination_owner(session)?;
        Ok(self.timers.set(project, owner, body, after)?)
    }

    /// Arms a fire-when-idle timer owned by the session's bound process: it delivers `body` to
    /// that process when the watched `processes` reach the `mode` idle quorum, or when `max_wait`
    /// elapses. Reports whether the condition is **already** satisfied and which processes it is
    /// still waiting on, read from the live idle state — a non-blocking signal. The watched
    /// processes need not be in scope: a timer only ever delivers to its own owner, and idle state
    /// is already open through the read tools, so watching another process observes nothing it
    /// could not already see.
    pub fn timer_fire_when_idle(
        &self,
        session: SessionId,
        body: String,
        processes: Vec<ProcessId>,
        mode: IdleMode,
        max_wait: Option<Duration>,
    ) -> Result<SetWhenIdleOutcome, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let owner = self.coordination_owner(session)?;
        let waiting_on: Vec<ProcessId> = processes
            .iter()
            .copied()
            .filter(|&process| !self.is_idle_now(process))
            .collect();
        let already_idle = mode.quorum_met(&processes, |process| self.is_idle_now(process));
        let timer = self
            .timers
            .set_when_idle(project, owner, body, processes, mode, max_wait)?;
        Ok(SetWhenIdleOutcome {
            timer,
            already_idle,
            waiting_on,
        })
    }

    /// Cancels a timer the session's bound process owns, returning whether one was removed.
    pub fn timer_cancel(
        &self,
        session: SessionId,
        timer: TimerId,
    ) -> Result<bool, CoordinationError> {
        let owner = self.coordination_owner(session)?;
        Ok(self.timers.cancel(timer, owner)?)
    }

    /// Pauses a timer the session's bound process owns (freezing the time that remains), returning
    /// whether one was paused.
    pub fn timer_pause(
        &self,
        session: SessionId,
        timer: TimerId,
    ) -> Result<bool, CoordinationError> {
        let owner = self.coordination_owner(session)?;
        Ok(self.timers.pause(timer, owner)?)
    }

    /// Resumes a paused timer the session's bound process owns (re-arming it with the time that
    /// remained), returning whether one was resumed.
    pub fn timer_resume(
        &self,
        session: SessionId,
        timer: TimerId,
    ) -> Result<bool, CoordinationError> {
        let owner = self.coordination_owner(session)?;
        Ok(self.timers.resume(timer, owner)?)
    }

    /// Every timer the session's bound process owns (armed or paused).
    pub fn timer_list(&self, session: SessionId) -> Result<Vec<TimerView>, CoordinationError> {
        let owner = self.coordination_owner(session)?;
        Ok(self.timers.list(owner)?)
    }

    /// Clears every stale timer on launch — see [`Timers::reconcile`](crate::coordination::Timers::reconcile).
    /// Not session-scoped; the composition root calls it once at startup.
    pub fn reconcile_timers(&self) -> Result<usize, StoreError> {
        self.timers.reconcile()
    }

    /// Whether a process counts as idle right now for a fire-when-idle timer — the snapshot the
    /// `already_idle`/`waiting_on` report is built from. Applies the same rule the scheduler fires
    /// on ([`watched_is_idle`]): the agent idle FSM (C4) reports `Idle`, or the process has left the
    /// registry (it can no longer work), so the report can never disagree with what fires.
    fn is_idle_now(&self, process: ProcessId) -> bool {
        watched_is_idle(
            self.idle.activity(process),
            self.supervisor.view(process).is_some(),
        )
    }

    /// The session's effective project, or [`CoordinationError::NoProjectScope`].
    fn coordination_scope(&self, session: SessionId) -> Result<ProjectId, CoordinationError> {
        self.effective_project(session)
            .ok_or(CoordinationError::NoProjectScope)
    }

    /// The session's bound process — the owner a lease or timer is attributed to — or
    /// [`CoordinationError::NoBoundProcess`].
    fn coordination_owner(&self, session: SessionId) -> Result<ProcessId, CoordinationError> {
        self.identity
            .origin(session)
            .process()
            .ok_or(CoordinationError::NoBoundProcess)
    }
}

#[cfg(test)]
#[path = "coordination_tests.rs"]
mod tests;
