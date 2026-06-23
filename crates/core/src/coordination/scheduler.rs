//! The timer scheduler (context C6): the self-supervised loop that fires due timers and delivers
//! their body to the owning process as a fresh turn.
//!
//! It is woken three ways and re-evaluates the full armed set on each: a [`Clock`] sleep until the
//! soonest deadline (for [`At`](super::timer::FireCond::At) and the idle max-wait backstops); a [`Notify`] the
//! [`Timers`](super::Timers) aggregate pings when a timer is created or resumed (so an
//! already-satisfied condition fires at once); and the [`DomainEvent`] bus, from which it tracks
//! each agent's idle state via [`AgentActivityChanged`](DomainEvent::AgentActivityChanged) — the
//! C4 idle signal, consumed as events so coordination depends only on the shared event type, not
//! on C4's internals. A due timer is claimed atomically (so a concurrent pause/cancel wins the
//! race cleanly) and its body is written to its owner's PTY — reusing the one supervisor input
//! behaviour, never reimplementing it. It holds a [`Weak`] reference to the supervisor so it never
//! keeps the app alive, and is self-supervised like the monitoring samplers: a panicking pass is
//! isolated and the loop restarts.

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::Notify;

use crate::agents::AgentActivity;
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::supervision::supervise;
use crate::supervisor::Supervisor;

use super::timer::watched_is_idle;
use super::timer_repo::{StoredTimer, TimerRepo};

/// Fires due coordination timers. Cloneable so the supervising [`run`](TimerScheduler::run) can
/// hand a fresh copy to each restart of the inner loop; all clones share the same repo, clock,
/// wake handle, and event bus.
#[derive(Clone)]
pub struct TimerScheduler {
    repo: Arc<dyn TimerRepo>,
    clock: Arc<dyn Clock>,
    wake: Arc<Notify>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
}

impl TimerScheduler {
    /// Builds a scheduler over the timer store, clock, the aggregate's wake handle, and the event
    /// bus, watching the given supervisor weakly (so it never keeps the app alive).
    pub(super) fn new(
        repo: Arc<dyn TimerRepo>,
        clock: Arc<dyn Clock>,
        wake: Arc<Notify>,
        bus: EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            repo,
            clock,
            wake,
            bus,
            supervisor,
        }
    }

    /// Runs the scheduler until the supervisor is dropped or the bus closes, supervising the inner
    /// loop so a panicking pass is isolated and restarted (see [`supervise`]). Returned for the
    /// composition root to spawn once on its runtime.
    pub async fn run(self) {
        let clock = self.clock.clone();
        supervise(clock, move || self.clone().schedule_loop()).await;
    }

    /// The scheduling loop: evaluate and fire every due timer, then wait for the soonest deadline,
    /// a wake, or an idle/removal event — re-evaluating on each. Tracks agent idle state from the
    /// bus; ends when the supervisor is dropped or the bus closes (app shutdown).
    async fn schedule_loop(self) {
        let mut events = self.bus.subscribe();
        // Last-known activity per agent, from the bus. Only agents ever appear here, so it stays
        // bounded to the live agent set; a process unknown here is treated as not-idle unless it
        // has left the registry entirely (see `is_idle`).
        let mut activity: HashMap<ProcessId, AgentActivity> = HashMap::new();
        loop {
            let Some(supervisor) = self.supervisor.upgrade() else {
                return;
            };
            let now = self.clock.now_unix_millis();
            let armed = self.repo.armed().unwrap_or_default();
            let mut next_deadline: Option<u64> = None;
            for timer in armed {
                if Self::is_due(&timer, now, &activity, &supervisor) {
                    // Claim atomically: a timer the owner paused or cancelled since we read the
                    // armed set is no longer claimable, so it is not fired.
                    if let Ok(Some(claimed)) = self.repo.take_if_armed(timer.id) {
                        deliver(&supervisor, claimed).await;
                    }
                } else {
                    next_deadline =
                        Some(next_deadline.map_or(timer.deadline_unix_millis, |soonest| {
                            soonest.min(timer.deadline_unix_millis)
                        }));
                }
            }
            // Drop the strong reference before idling, so the loop never keeps the app alive while
            // it waits.
            drop(supervisor);

            tokio::select! {
                result = events.recv() => match result {
                    Err(RecvError::Closed) => return,
                    // A lagged subscriber may have missed an idle transition; re-evaluate.
                    Err(RecvError::Lagged(_)) => {}
                    Ok(DomainEvent::AgentActivityChanged { id, state }) => {
                        activity.insert(id, state);
                    }
                    Ok(DomainEvent::ProcessRemoved { id }) => {
                        activity.remove(&id);
                        // A closed process strands no timers: drop the ones it owned.
                        let _ = self.repo.release_owner(id);
                    }
                    Ok(_) => {}
                },
                () = self.wake.notified() => {}
                () = sleep_until_millis(&self.clock, next_deadline) => {}
            }
        }
    }

    /// Whether `timer` should fire now: any timer once its deadline passes (its scheduled time, or
    /// a fire-when-idle backstop), and a fire-when-idle timer as soon as its watched quorum is idle.
    /// The quorum and the per-process idle rule are the shared `IdleMode::quorum_met` and
    /// [`watched_is_idle`], so this fires on exactly what the façade reports at set time.
    fn is_due(
        timer: &StoredTimer,
        now: u64,
        activity: &HashMap<ProcessId, AgentActivity>,
        supervisor: &Supervisor,
    ) -> bool {
        if timer.deadline_unix_millis <= now {
            return true;
        }
        match timer.fire.idle_quorum() {
            None => false,
            Some((mode, watched)) => mode.quorum_met(watched, |p| {
                watched_is_idle(activity.get(&p).copied(), supervisor.view(p).is_some())
            }),
        }
    }
}

/// Delivers a fired timer's body to its owner as a fresh user turn: the body followed by a
/// carriage return so the agent CLI submits it. Best-effort — the timer is already claimed and
/// removed, so an owner that has since gone simply means the body is not delivered.
async fn deliver(supervisor: &Supervisor, timer: StoredTimer) {
    let mut input = timer.body.into_bytes();
    input.push(b'\r');
    let _ = supervisor.write_stdin(timer.owner, input).await;
}

/// Sleeps until the absolute `deadline` (Unix milliseconds) per the clock, or forever when none is
/// pending — so the scheduler idles without arming a timer whenever nothing is due. The wait is
/// driven by the clock's monotonic [`sleep`](Clock::sleep), which a mock advances in lockstep with
/// its wall clock, so deadline firing stays deterministic in tests.
async fn sleep_until_millis(clock: &Arc<dyn Clock>, deadline: Option<u64>) {
    match deadline {
        Some(at) => {
            let remaining = at.saturating_sub(clock.now_unix_millis());
            clock.sleep(Duration::from_millis(remaining)).await;
        }
        None => std::future::pending::<()>().await,
    }
}

#[cfg(test)]
#[path = "scheduler_tests.rs"]
mod tests;
