//! Timer actions (context C8 → C6): the plain and fire-when-idle timer surface a remote caller
//! (MCP today) drives within its effective project, plus the local, trusted Tauri surface that
//! already knows the owning process.
//!
//! A timer is project-scoped and process-owned, so the session-scoped methods resolve the
//! session's **effective project** and its **bound process** — reusing
//! [`coordination_scope`](Facade::coordination_scope) and
//! [`coordination_owner`](Facade::coordination_owner), shared with the lease and scratchpad
//! surface — before routing to the one [`Timers`](crate::coordination::Timers) aggregate; the
//! local surface is handed the owner directly instead, since the local UI already has full
//! project access. The bound process must be authentic (checked at bind time), which is what a
//! fired timer delivers its body to when that process closes. A fire-when-idle timer is also
//! enriched here with a live read of the agent idle FSM (C4), so its `already_idle`/`waiting_on`
//! report never disagrees with what the scheduler actually fires on.

use std::time::Duration;

use super::coordination::check_payload_size;
use super::scoped::ScopedFacade;
use super::Facade;
use crate::coordination::{
    watched_is_idle, IdleMode, SetWhenIdleOutcome, TimerView, MAX_TIMER_BODY_BYTES,
};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::{ProcessId, TimerId};
use crate::ports::StoreError;

impl Facade {
    /// Cancels a timer owned by `owner` — the local, trusted Tauri surface passes the owner
    /// process id directly (no session scope needed; the local UI already has full project access).
    pub fn timer_cancel_for(&self, owner: ProcessId, timer: TimerId) -> Result<bool, StoreError> {
        let cancelled = self.timers.cancel(timer, owner)?;
        if cancelled {
            self.bus
                .publish(DomainEvent::TimerCleared { owner, id: timer });
        }
        Ok(cancelled)
    }

    /// Pauses a timer owned by `owner` — the local, trusted Tauri surface.
    pub fn timer_pause_for(&self, owner: ProcessId, timer: TimerId) -> Result<bool, StoreError> {
        let paused = self.timers.pause(timer, owner)?;
        if paused {
            self.bus
                .publish(DomainEvent::TimerPaused { owner, id: timer });
        }
        Ok(paused)
    }

    /// Resumes a paused timer owned by `owner` — the local, trusted Tauri surface.
    pub fn timer_resume_for(&self, owner: ProcessId, timer: TimerId) -> Result<bool, StoreError> {
        let resumed = self.timers.resume(timer, owner)?;
        if resumed {
            self.bus
                .publish(DomainEvent::TimerResumed { owner, id: timer });
        }
        Ok(resumed)
    }

    /// Clears every stale timer on launch — see [`Timers::reconcile`](crate::coordination::Timers::reconcile).
    /// Not session-scoped; the composition root calls it once at startup.
    pub fn reconcile_timers(&self) -> Result<usize, StoreError> {
        self.timers.reconcile()
    }

    /// Whether a process counts as idle right now for a fire-when-idle timer — the snapshot the
    /// `already_idle`/`waiting_on` report is built from. Applies the same rule the scheduler fires
    /// on ([`watched_is_idle`]): the agent idle FSM (C4) reports `Idle`, or the process has left the
    /// registry (it can no longer work), so the report can never disagree with what fires. Shared
    /// with the sibling orchestration surface ([`super::orchestration`]), so every timer read
    /// tests idle the same way.
    pub(in crate::facade) fn is_idle_now(&self, process: ProcessId) -> bool {
        watched_is_idle(
            self.idle.activity(process),
            self.supervisor.view(process).is_some(),
        )
    }
}

impl ScopedFacade<'_> {
    /// Arms a plain timer in the session's effective project, owned by its bound process, that
    /// delivers `body` to that process as a fresh turn after `after` (immediately when `None`).
    /// Needs a bound process — the owner the body is delivered to and that the timer is cleaned up
    /// with on close.
    pub fn timer_set(
        &self,
        body: String,
        after: Option<Duration>,
    ) -> Result<TimerView, CoordinationError> {
        let project = self.coordination_scope()?;
        let owner = self.coordination_owner()?;
        check_payload_size(body.len(), MAX_TIMER_BODY_BYTES, "timer body")?;
        let timer = self.inner.timers.set(project, owner, body, after)?;
        self.inner.bus.publish(DomainEvent::TimerArmed {
            owner,
            id: timer.id,
        });
        Ok(timer)
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
        body: String,
        processes: Vec<ProcessId>,
        mode: IdleMode,
        max_wait: Option<Duration>,
    ) -> Result<SetWhenIdleOutcome, CoordinationError> {
        let project = self.coordination_scope()?;
        let owner = self.coordination_owner()?;
        check_payload_size(body.len(), MAX_TIMER_BODY_BYTES, "timer body")?;
        let (waiting_on, already_idle) =
            mode.idle_report(&processes, |process| self.inner.is_idle_now(process));
        let mut timer = self
            .inner
            .timers
            .set_when_idle(project, owner, body, processes, mode, max_wait)?;
        timer.waiting_on = waiting_on;
        timer.already_idle = already_idle;
        self.inner.bus.publish(DomainEvent::TimerArmed {
            owner,
            id: timer.id,
        });
        Ok(SetWhenIdleOutcome { timer })
    }

    /// Cancels a timer the session's bound process owns, returning whether one was removed.
    pub fn timer_cancel(&self, timer: TimerId) -> Result<bool, CoordinationError> {
        let owner = self.coordination_owner()?;
        let cancelled = self.inner.timers.cancel(timer, owner)?;
        if cancelled {
            self.inner
                .bus
                .publish(DomainEvent::TimerCleared { owner, id: timer });
        }
        Ok(cancelled)
    }

    /// Pauses a timer the session's bound process owns (freezing the time that remains), returning
    /// whether one was paused.
    pub fn timer_pause(&self, timer: TimerId) -> Result<bool, CoordinationError> {
        let owner = self.coordination_owner()?;
        let paused = self.inner.timers.pause(timer, owner)?;
        if paused {
            self.inner
                .bus
                .publish(DomainEvent::TimerPaused { owner, id: timer });
        }
        Ok(paused)
    }

    /// Resumes a paused timer the session's bound process owns (re-arming it with the time that
    /// remained), returning whether one was resumed.
    pub fn timer_resume(&self, timer: TimerId) -> Result<bool, CoordinationError> {
        let owner = self.coordination_owner()?;
        let resumed = self.inner.timers.resume(timer, owner)?;
        if resumed {
            self.inner
                .bus
                .publish(DomainEvent::TimerResumed { owner, id: timer });
        }
        Ok(resumed)
    }

    /// Every timer the session's bound process owns (armed or paused), each fire-when-idle timer
    /// enriched with its live `waiting_on`/`already_idle` from the current idle state — the same
    /// enrichment [`timer_fire_when_idle`](Self::timer_fire_when_idle) reports at set time, so a
    /// caller polling this instead of waiting on the timer sees the same answer.
    pub fn timer_list(&self) -> Result<Vec<TimerView>, CoordinationError> {
        let owner = self.coordination_owner()?;
        Ok(self
            .inner
            .timers
            .list(owner)?
            .into_iter()
            .map(|mut view| {
                let enrichment = view.fire.idle_quorum().map(|(mode, watched)| {
                    mode.idle_report(watched, |process| self.inner.is_idle_now(process))
                });
                if let Some((waiting_on, already_idle)) = enrichment {
                    view.waiting_on = waiting_on;
                    view.already_idle = already_idle;
                }
                view
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "timer_tests.rs"]
mod tests;
