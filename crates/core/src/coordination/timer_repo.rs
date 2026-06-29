//! The durable store of coordination timers and the port over it (context C6).
//!
//! Like the lease port, this is deliberately small and **atomic**: every operation that depends
//! on a timer's current state — claiming an armed timer to fire it, pausing, resuming, cancelling
//! — is one indivisible method the adapter performs under a single guard, never a read the
//! aggregate then acts on, so the [`TimerScheduler`](super::TimerScheduler) firing a timer and its
//! owner pausing or cancelling it cannot interleave to fire a timer that was just suspended. The
//! bounded context owns its own port (with a [`NoopTimerRepo`] default) rather than the shared
//! ports module, so coordination persistence stays confined to coordination.

use super::timer::{FireCond, TimerStatus, TimerView};
use crate::ids::{ProcessId, ProjectId, TimerId};
use crate::ports::StoreError;

/// A timer to persist: everything but the store-assigned id. The `deadline_unix_millis` is the
/// absolute wall-clock time the timer fires (for [`FireCond::At`]) or its max-wait backstop (for
/// the idle conditions), kept beside the condition so pausing/resuming can re-arm it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTimer {
    pub project: ProjectId,
    pub owner: ProcessId,
    pub body: String,
    pub fire: FireCond,
    pub deadline_unix_millis: u64,
}

/// A persisted timer: a [`NewTimer`] plus its store-assigned [`TimerId`], current [`TimerStatus`],
/// and — when paused — the milliseconds that remained at the moment it was paused, so resuming can
/// re-arm it with the same time left. Times are Unix milliseconds (a persistable wall clock) so a
/// deadline written on one run is comparable when read on a later one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTimer {
    pub id: TimerId,
    pub project: ProjectId,
    pub owner: ProcessId,
    pub body: String,
    pub fire: FireCond,
    pub deadline_unix_millis: u64,
    pub status: TimerStatus,
    pub remaining_on_pause_millis: Option<u64>,
}

impl StoredTimer {
    /// Projects this row to the caller-facing [`TimerView`], carrying the paused remainder through
    /// as [`paused_remaining_millis`](TimerView::paused_remaining_millis) so a paused timer reports
    /// its frozen remaining time. `waiting_on` and `already_idle` default to their empty/false
    /// sentinels; the façade enriches them from live idle state before returning to a caller.
    pub fn into_view(self) -> TimerView {
        TimerView {
            id: self.id,
            owner: self.owner,
            body: self.body,
            fire: self.fire,
            status: self.status,
            deadline_unix_millis: self.deadline_unix_millis,
            waiting_on: Vec::new(),
            already_idle: false,
            paused_remaining_millis: self.remaining_on_pause_millis,
        }
    }
}

/// Durable repository of coordination timers. One row per timer, keyed by a store-assigned
/// [`TimerId`]. Every state-dependent method is atomic with respect to the others, so the
/// scheduler claiming a timer to fire it can never race an owner pausing or cancelling it.
pub trait TimerRepo: Send + Sync {
    /// Persists a new **armed** timer and returns its assigned id.
    fn create(&self, timer: &NewTimer) -> Result<TimerId, StoreError>;

    /// Every **armed** timer (paused ones excluded), for the scheduler to evaluate. A plain read;
    /// the scheduler decides which are due and then claims each atomically with
    /// [`take_if_armed`](Self::take_if_armed).
    fn armed(&self) -> Result<Vec<StoredTimer>, StoreError>;

    /// Atomically claims the timer `id` for firing — removing it and returning it — but only if
    /// it is still **armed**. Returns `None` if it was paused or already gone (cancelled, fired,
    /// or its owner closed) since the scheduler read it, so a suspended timer is never fired.
    fn take_if_armed(&self, id: TimerId) -> Result<Option<StoredTimer>, StoreError>;

    /// Cancels (removes) the timer `id` only if `owner` holds it, returning whether one was
    /// removed. The ownership check is part of the removal, so a caller cannot cancel another's
    /// timer.
    fn cancel(&self, id: TimerId, owner: ProcessId) -> Result<bool, StoreError>;

    /// Pauses the timer `id` if `owner` holds it and it is armed, recording the milliseconds that
    /// remain until its deadline (`deadline - now`, never negative) so resuming can restore them;
    /// returns whether one was paused. Atomic: the armed check and the freeze are one step.
    fn pause(&self, id: TimerId, owner: ProcessId, now: u64) -> Result<bool, StoreError>;

    /// Resumes the timer `id` if `owner` holds it and it is paused, re-arming it with a deadline of
    /// `now + remaining`; returns whether one was resumed. Atomic: the paused check and the re-arm
    /// are one step.
    fn resume(&self, id: TimerId, owner: ProcessId, now: u64) -> Result<bool, StoreError>;

    /// Every timer in `project` (armed or paused), ordered by id — the project-scoped read the
    /// orchestration snapshot projects. A plain read; unlike [`list`](Self::list) it is keyed by
    /// project, not owner, so the snapshot shows every timer in the project regardless of which
    /// process owns it.
    fn list_in_project(&self, project: ProjectId) -> Result<Vec<StoredTimer>, StoreError>;

    /// Every timer `owner` holds (armed or paused) — the rows `timer_list` returns.
    fn list(&self, owner: ProcessId) -> Result<Vec<StoredTimer>, StoreError>;

    /// Removes every timer owned by `owner` — the process closed — returning how many. The
    /// scheduler calls this when it sees the owner leave, so a dead owner strands no timers.
    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError>;

    /// Clears every timer — launch reconciliation (see [`Timers::reconcile`](super::Timers::reconcile)).
    /// Returns how many were cleared.
    fn clear(&self) -> Result<usize, StoreError>;
}

/// A [`TimerRepo`] that stores nothing — the default until the durable adapter is wired, so the
/// core runs (timers simply never persist or fire) without it. Creating returns a placeholder id
/// and every read is empty.
#[derive(Clone, Copy, Default)]
pub struct NoopTimerRepo;

impl TimerRepo for NoopTimerRepo {
    fn create(&self, _timer: &NewTimer) -> Result<TimerId, StoreError> {
        Ok(TimerId::from_raw(0))
    }
    fn armed(&self) -> Result<Vec<StoredTimer>, StoreError> {
        Ok(Vec::new())
    }
    fn take_if_armed(&self, _id: TimerId) -> Result<Option<StoredTimer>, StoreError> {
        Ok(None)
    }
    fn cancel(&self, _id: TimerId, _owner: ProcessId) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn pause(&self, _id: TimerId, _owner: ProcessId, _now: u64) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn resume(&self, _id: TimerId, _owner: ProcessId, _now: u64) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn list_in_project(&self, _project: ProjectId) -> Result<Vec<StoredTimer>, StoreError> {
        Ok(Vec::new())
    }
    fn list(&self, _owner: ProcessId) -> Result<Vec<StoredTimer>, StoreError> {
        Ok(Vec::new())
    }
    fn release_owner(&self, _owner: ProcessId) -> Result<usize, StoreError> {
        Ok(0)
    }
    fn clear(&self) -> Result<usize, StoreError> {
        Ok(0)
    }
}
