//! The timer aggregate (context C6): process-owned timers that deliver a body to their owner
//! as a fresh turn when they fire.
//!
//! A timer fires one of two ways: at an absolute wall-clock deadline ([`FireCond::At`]), or when
//! the agent processes it watches go idle ([`FireCond::WhenIdleAny`]/[`WhenIdleAll`](FireCond::WhenIdleAll))
//! — the token-free "wait until the workers are done" primitive — with the deadline serving as a
//! max-wait backstop so a watched process that never settles cannot block the timer forever. On
//! fire the [`TimerScheduler`](super::TimerScheduler) delivers the stored `body` to the owning
//! process verbatim, as a fresh user turn. Firing is one-shot: a fired timer is gone.
//!
//! The aggregate owns the *policy* (the delay/max-wait defaults and ceilings) and turns a relative
//! delay into the absolute, persistable deadline the durable [`TimerRepo`] stores. Setting (or
//! resuming) a timer nudges the scheduler through a shared [`Notify`] so an already-satisfied
//! condition fires promptly rather than waiting for the next event.

use std::sync::{Arc, Weak};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use super::timer_repo::{NewTimer, TimerRepo};
use crate::events::EventBus;
use crate::idle::{AgentActivity, ObservedActivities, ObservedActivity};
use crate::ids::{ProcessId, ProjectId, TimerId};
use crate::ports::{Clock, StoreError};
use crate::process::ProcStatus;
use crate::supervisor::Supervisor;

/// The ceiling on an [`FireCond::At`] timer's delay — a bound (per the longevity rules) so a
/// caller cannot schedule one absurdly far out. A request above this is clamped. There is no
/// floor: a zero (or omitted) delay is legitimate — "deliver this to me now, as a fresh turn".
const MAX_TIMER_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

/// The most a timer's body may hold, in bytes. A body is a short instruction delivered to the
/// owning agent as one fresh turn, not a document; this bounds it so a runaway caller cannot grow
/// the table through one write. Enforced by the facade, the one write path, before persistence.
pub const MAX_TIMER_BODY_BYTES: usize = 16 * 1024;

/// The max-wait backstop applied to a fire-when-idle timer when the caller names none, so a
/// watched process that never goes idle (a stuck agent) cannot leave the timer waiting forever —
/// it fires after this regardless. Long enough to outlast a real piece of work.
const DEFAULT_IDLE_MAX_WAIT: Duration = Duration::from_secs(60 * 60);

/// The ceiling on a fire-when-idle max-wait, clamped like the [`At`](FireCond::At) delay so the
/// backstop itself stays bounded.
const MAX_IDLE_MAX_WAIT: Duration = Duration::from_secs(24 * 60 * 60);

/// What a timer is waiting for. The `At` deadline and each idle backstop are absolute Unix
/// milliseconds — a persistable wall clock ([`Clock::now_unix_millis`]) — kept *outside* this
/// shape (on the row and the views) so pausing can freeze and resuming can re-arm the deadline
/// without rewriting the condition. Tagged so the wire and the stored JSON name the variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FireCond {
    /// Fire when the deadline passes — a plain scheduled delivery.
    At,
    /// Fire when **any** watched process is idle (or the max-wait backstop passes).
    WhenIdleAny { watched: Vec<ProcessId> },
    /// Fire when **all** watched processes are idle (or the max-wait backstop passes).
    WhenIdleAll { watched: Vec<ProcessId> },
}

impl FireCond {
    /// The idle quorum and watched set this condition fires on, or `None` for a plain [`At`](Self::At)
    /// timer (which fires only at its deadline). Lets the scheduler evaluate an idle condition
    /// through the one [`IdleMode::quorum_met`], rather than re-matching the variants itself.
    pub(crate) fn idle_quorum(&self) -> Option<(IdleMode, &[ProcessId])> {
        match self {
            FireCond::At => None,
            FireCond::WhenIdleAny { watched } => Some((IdleMode::Any, watched)),
            FireCond::WhenIdleAll { watched } => Some((IdleMode::All, watched)),
        }
    }
}

/// Which idle quorum a fire-when-idle timer needs — the one knob distinguishing the
/// `timer_fire_when_idle_any` and `_all` tools, so one aggregate method serves both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleMode {
    /// Fire as soon as one watched process is idle.
    Any,
    /// Fire only once every watched process is idle.
    All,
}

impl IdleMode {
    /// Whether the idle quorum is met across `watched`, given a per-process idle test: `Any` as
    /// soon as one watched process is idle, `All` only once every one is. Neither is met by an
    /// empty set — an `All` timer with nothing to watch fires on its backstop, not at once. The
    /// single definition of the quorum, shared by the scheduler (deciding to fire) and the façade
    /// (reporting `already_idle` at set time), so the two can never disagree.
    pub(crate) fn quorum_met(
        self,
        watched: &[ProcessId],
        is_idle: impl Fn(ProcessId) -> bool,
    ) -> bool {
        match self {
            IdleMode::Any => watched.iter().any(|&p| is_idle(p)),
            IdleMode::All => !watched.is_empty() && watched.iter().all(|&p| is_idle(p)),
        }
    }
}

/// Whether a watched process counts as idle for a fire-when-idle timer, from what has been
/// [observed](ObservedActivity) of its work and its supervision status (`None` once it has left the
/// registry). Only positive evidence that the work is over counts:
///
/// - a process that has left the registry can no longer do anything, so it counts as done however
///   it was last classified, and a departed worker never deadlocks the wait;
/// - a live process ([`ProcStatus::is_active`]) counts as idle once the agent idle FSM reports
///   [`Idle`](AgentActivity::Idle) for a turn it was observed starting — the quiet of a CLI still
///   starting up is not a finished turn, and neither is a process working, blocked on a prompt, or
///   never classified at all;
/// - a process at rest counts as done only if it was launched, because that is the one thing that
///   separates the two ways to be at rest: a worker that ran and ended can never become busy again,
///   while one nobody has started yet has not begun. They share a status, so status alone would
///   read every unstarted process in the project as finished.
///
/// A process with no positive evidence either way — never launched, or launched but silent — is
/// not idle: the wait continues, with the timer's backstop as the guarantee it eventually fires.
fn watched_is_idle(observed: ObservedActivity, status: Option<ProcStatus>) -> bool {
    match status {
        None => true,
        Some(status) if status.is_active() => observed.latest().is_some_and(AgentActivity::is_idle),
        Some(_) => observed.has_launched(),
    }
}

/// Whether `process` counts as idle for a fire-when-idle timer right now, applying
/// [`watched_is_idle`] to the two live registries: what has been observed of its work (C4) and its
/// supervision status (C2).
///
/// The one place those reads are composed. The scheduler deciding to fire and the façade reporting
/// `already_idle`/`waiting_on` both ask through here, over the same registry instances, so a caller
/// is never told one thing while another fires — and neither of them keeps a copy that could go
/// stale between the two.
pub(crate) fn watched_process_is_idle(
    idle: &dyn ObservedActivities,
    supervisor: &Supervisor,
    process: ProcessId,
) -> bool {
    watched_is_idle(
        idle.observed_activity(process),
        supervisor.view(process).map(|view| view.status),
    )
}

/// Whether a timer is counting down or has been suspended by its owner. A paused timer never
/// fires; resuming re-arms it with the time that remained when it was paused (see
/// [`TimerRepo::resume`](super::TimerRepo::resume)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    /// Counting down toward its deadline / watching for idle.
    Armed,
    /// Suspended by its owner, holding the time that remained; will not fire until resumed.
    Paused,
}

/// A timer as a caller sees it (the answer to setting one and the rows `timer_list` returns):
/// its id, the body it will deliver, what it is waiting for, when its deadline is, and whether
/// it is armed or paused. `waiting_on` and `already_idle` are not stored: they default to
/// empty/false on the shape the aggregate builds and are filled in by [`report_idle`]
/// (Self::report_idle), which every façade read of a timer goes through, from the live idle state.
/// Built from a [`StoredTimer`](super::StoredTimer) so the wire shape cannot drift from the
/// persisted one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerView {
    pub id: TimerId,
    /// The process that owns this timer — the one its body is delivered to on fire.
    pub owner: ProcessId,
    pub body: String,
    pub fire: FireCond,
    pub status: TimerStatus,
    pub deadline_unix_millis: u64,
    /// Watched processes not yet idle — computed from live idle state at read time. Empty for a
    /// plain [`FireCond::At`] timer and after the quorum is met.
    #[serde(default)]
    pub waiting_on: Vec<ProcessId>,
    /// Whether the idle condition was already satisfied at the moment this view was built. Only
    /// ever `true` for a fire-when-idle timer whose quorum was met at read time.
    #[serde(default)]
    pub already_idle: bool,
    /// For a paused timer, the milliseconds that remained when it was paused — the frozen value a
    /// caller shows so a paused countdown does not drift with the wall clock. `None` for an armed
    /// timer, whose remaining time is derived from [`deadline_unix_millis`](Self::deadline_unix_millis).
    #[serde(default)]
    pub paused_remaining_millis: Option<u64>,
}

impl TimerView {
    /// Fills this view's read-time idle report — which watched processes are not yet idle, and
    /// whether the quorum is already met — from the live `is_idle` test. Leaves a plain
    /// [`FireCond::At`] timer untouched: it watches nothing. The one place the report is computed,
    /// so every view of a timer carries the same answer as the aggregate it is reported beside.
    pub(crate) fn report_idle(&mut self, is_idle: impl Fn(ProcessId) -> bool) {
        let Some((mode, watched)) = self.fire.idle_quorum() else {
            return;
        };
        self.waiting_on = watched.iter().copied().filter(|&p| !is_idle(p)).collect();
        self.already_idle = mode.quorum_met(watched, is_idle);
    }
}

/// The outcome of arming a fire-when-idle timer: the timer itself, whether its idle condition is
/// **already** satisfied at set time (so it will fire promptly), and which watched processes it
/// is still `waiting_on` (those not yet idle). A non-blocking signal the caller can act on. The
/// timer view carries the same report ([`TimerView::report_idle`]) — one answer, not two.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetWhenIdleOutcome {
    pub timer: TimerView,
    pub already_idle: bool,
    pub waiting_on: Vec<ProcessId>,
}

/// The timer aggregate over the durable [`TimerRepo`] and the [`Clock`]. The repo persists and
/// makes each state-dependent step atomic; the clock supplies the persistable now the deadlines
/// are computed from. The [`Notify`] is shared with the [`TimerScheduler`](super::TimerScheduler)
/// so creating or resuming a timer wakes it to re-evaluate at once. Cheap to share via its `Arc`s.
pub struct Timers {
    repo: Arc<dyn TimerRepo>,
    clock: Arc<dyn Clock>,
    wake: Arc<Notify>,
}

impl Timers {
    /// Builds the aggregate over its durable store, clock, and the scheduler-wake handle.
    pub fn new(repo: Arc<dyn TimerRepo>, clock: Arc<dyn Clock>, wake: Arc<Notify>) -> Self {
        Self { repo, clock, wake }
    }

    /// Builds the [`TimerScheduler`](super::TimerScheduler) over this aggregate's store, clock,
    /// and wake handle — so the scheduler and the aggregate share one repo and one wake signal.
    /// `idle` must be the same observation registry the façade reports a timer from, so a timer
    /// fires on exactly the state its caller was told about. The composition root spawns the
    /// returned scheduler's loop once.
    pub(crate) fn scheduler(
        &self,
        bus: EventBus,
        supervisor: Weak<Supervisor>,
        idle: Arc<dyn ObservedActivities>,
    ) -> super::TimerScheduler {
        super::TimerScheduler::new(
            self.repo.clone(),
            self.clock.clone(),
            self.wake.clone(),
            bus,
            supervisor,
            idle,
        )
    }

    /// Arms a plain timer that delivers `body` to `owner` after `after` (the [ceiling]
    /// (MAX_TIMER_DELAY) when larger; immediately when `None` or zero). Wakes the scheduler.
    pub fn set(
        &self,
        project: ProjectId,
        owner: ProcessId,
        body: String,
        after: Option<Duration>,
    ) -> Result<TimerView, StoreError> {
        let now = self.clock.now_unix_millis();
        let delay = after.unwrap_or_default().min(MAX_TIMER_DELAY).as_millis() as u64;
        self.arm(
            project,
            owner,
            body,
            FireCond::At,
            now.saturating_add(delay),
        )
    }

    /// Arms a fire-when-idle timer that delivers `body` to `owner` when the `watched` processes
    /// reach the `mode` idle quorum, or when the `max_wait` backstop passes (the [default]
    /// (DEFAULT_IDLE_MAX_WAIT) when `None`, [clamped](MAX_IDLE_MAX_WAIT) otherwise). Wakes the
    /// scheduler. The `already_idle`/`waiting_on` report is computed by the caller (the façade,
    /// which holds the idle state); this only persists and arms.
    pub fn set_when_idle(
        &self,
        project: ProjectId,
        owner: ProcessId,
        body: String,
        watched: Vec<ProcessId>,
        mode: IdleMode,
        max_wait: Option<Duration>,
    ) -> Result<TimerView, StoreError> {
        let now = self.clock.now_unix_millis();
        let backstop = max_wait
            .unwrap_or(DEFAULT_IDLE_MAX_WAIT)
            .min(MAX_IDLE_MAX_WAIT)
            .as_millis() as u64;
        let fire = match mode {
            IdleMode::Any => FireCond::WhenIdleAny { watched },
            IdleMode::All => FireCond::WhenIdleAll { watched },
        };
        self.arm(project, owner, body, fire, now.saturating_add(backstop))
    }

    /// Cancels the timer `id` if it is owned by `owner`, returning whether one was removed. A
    /// caller cannot cancel a timer it does not own.
    pub fn cancel(&self, id: TimerId, owner: ProcessId) -> Result<bool, StoreError> {
        self.repo.cancel(id, owner)
    }

    /// Pauses the timer `id` if `owner` holds it and it is armed, freezing the time that remains;
    /// returns whether one was paused. A paused timer never fires until resumed.
    pub fn pause(&self, id: TimerId, owner: ProcessId) -> Result<bool, StoreError> {
        self.repo.pause(id, owner, self.clock.now_unix_millis())
    }

    /// Resumes the paused timer `id` if `owner` holds it, re-arming it with the time that
    /// remained when it was paused; returns whether one was resumed. Wakes the scheduler so a
    /// timer whose remaining time has already elapsed fires at once.
    pub fn resume(&self, id: TimerId, owner: ProcessId) -> Result<bool, StoreError> {
        let resumed = self.repo.resume(id, owner, self.clock.now_unix_millis())?;
        if resumed {
            self.wake.notify_one();
        }
        Ok(resumed)
    }

    /// Every timer `owner` holds (armed or paused), as views — the rows `timer_list` returns.
    pub fn list(&self, owner: ProcessId) -> Result<Vec<TimerView>, StoreError> {
        Ok(self
            .repo
            .list(owner)?
            .into_iter()
            .map(|stored| stored.into_view())
            .collect())
    }

    /// Every timer in `project` (armed or paused), as views, ordered by id — the read the
    /// orchestration snapshot projects. Keyed by project, not owner, so the snapshot shows every
    /// timer in the project regardless of which process owns it.
    pub fn list_project(&self, project: ProjectId) -> Result<Vec<TimerView>, StoreError> {
        Ok(self
            .repo
            .list_in_project(project)?
            .into_iter()
            .map(|stored| stored.into_view())
            .collect())
    }

    /// Clears every timer — launch reconciliation. Like a lease, a timer is process-owned and
    /// per-run process ids are recycled, so a timer left by a previous run names an owner that no
    /// longer exists and could never deliver; clearing the table on launch is the safe reconcile.
    /// Returns how many were cleared.
    pub fn reconcile(&self) -> Result<usize, StoreError> {
        self.repo.clear()
    }

    /// Persists a new armed timer and wakes the scheduler, returning the created view.
    fn arm(
        &self,
        project: ProjectId,
        owner: ProcessId,
        body: String,
        fire: FireCond,
        deadline_unix_millis: u64,
    ) -> Result<TimerView, StoreError> {
        let new = NewTimer {
            project,
            owner,
            body,
            fire,
            deadline_unix_millis,
        };
        let id = self.repo.create(&new)?;
        self.wake.notify_one();
        Ok(TimerView {
            id,
            owner,
            body: new.body,
            fire: new.fire,
            status: TimerStatus::Armed,
            deadline_unix_millis,
            waiting_on: Vec::new(),
            already_idle: false,
            paused_remaining_millis: None,
        })
    }
}

#[cfg(test)]
#[path = "timer_tests.rs"]
mod tests;
