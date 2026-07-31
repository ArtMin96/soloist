//! The timer scheduler (context C6): the self-supervised loop that fires due timers and delivers
//! their body to the owning process as a fresh turn.
//!
//! It is woken three ways and re-evaluates the full armed set on each: a [`Clock`] sleep until the
//! soonest deadline (for [`At`](super::timer::FireCond::At) and the idle max-wait backstops); a [`Notify`] the
//! [`Timers`](super::Timers) aggregate pings when a timer is created or resumed (so an
//! already-satisfied condition fires at once); and the [`DomainEvent`] bus, which tells it *when*
//! to look again — an agent's activity changed, a process started or exited, one left the registry.
//! *What* it then sees is read from the shared [`ObservedActivities`] registry (C4) and the process
//! registry (C2), never folded into state of its own, so the answer a timer fires on is the same
//! one its caller was given when the timer was armed, and survives both a supervised restart of
//! this loop and a dropped event. A due timer is claimed atomically (so a concurrent pause/cancel
//! wins the race cleanly) and its body is written to its owner's PTY — reusing the one supervisor
//! input behaviour, never reimplementing it. It holds a [`Weak`] reference to the supervisor so it
//! never keeps the app alive, and is self-supervised like the monitoring samplers: a panicking pass
//! is isolated and the loop restarts.

use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::Notify;

use crate::events::{DomainEvent, EventBus};
use crate::idle::ObservedActivities;
use crate::ports::Clock;
use crate::supervision::supervise;
use crate::supervisor::Supervisor;
use crate::turn::submitted_turn;

use super::timer::{watched_process_is_idle, FireCond};
use super::timer_repo::{StoredTimer, TimerRepo};

/// Fires due coordination timers. Cloneable so the supervising [`run`](TimerScheduler::run) can
/// hand a fresh copy to each restart of the inner loop; all clones share the same repo, clock,
/// wake handle, event bus, and idle registry.
#[derive(Clone)]
pub struct TimerScheduler {
    repo: Arc<dyn TimerRepo>,
    clock: Arc<dyn Clock>,
    wake: Arc<Notify>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
    idle: Arc<dyn ObservedActivities>,
}

impl TimerScheduler {
    /// Builds a scheduler over the timer store, clock, the aggregate's wake handle, the event bus,
    /// and the idle observation registry, watching the given supervisor weakly (so it never keeps
    /// the app alive).
    pub(super) fn new(
        repo: Arc<dyn TimerRepo>,
        clock: Arc<dyn Clock>,
        wake: Arc<Notify>,
        bus: EventBus,
        supervisor: Weak<Supervisor>,
        idle: Arc<dyn ObservedActivities>,
    ) -> Self {
        Self {
            repo,
            clock,
            wake,
            bus,
            supervisor,
            idle,
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
    /// a wake, or a domain event — re-evaluating from the live registries on each. Ends when the
    /// supervisor is dropped or the bus closes (app shutdown).
    async fn schedule_loop(self) {
        let mut events = self.bus.subscribe();
        loop {
            let Some(supervisor) = self.supervisor.upgrade() else {
                return;
            };
            let now = self.clock.now_unix_millis();
            let armed = self.repo.armed().unwrap_or_default();
            let mut next_deadline: Option<u64> = None;
            for timer in armed {
                if self.is_due(&timer, now, &supervisor) {
                    // Claim atomically: a timer the owner paused or cancelled since we read the
                    // armed set is no longer claimable, so it is not fired.
                    if let Ok(Some(claimed)) = self.repo.take_if_armed(timer.id) {
                        // The timer fired (it is claimed and removed); announce it before delivery
                        // so the wake-cycle UI sees it leave the armed set even when delivery is a
                        // best-effort no-op (the owner has since closed).
                        self.bus.publish(DomainEvent::TimerFired {
                            owner: claimed.owner,
                            id: claimed.id,
                        });
                        // Determine whether the backstop fired rather than the idle quorum: the
                        // deadline is computed before the idle check inside `is_due`, so a timer
                        // whose deadline passed but whose quorum was also met is correctly labelled
                        // as "deadline" (the backstop triggered the evaluation).
                        let fired_at_backstop = timer.deadline_unix_millis <= now;
                        deliver(&supervisor, claimed, fired_at_backstop);
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
                    Ok(DomainEvent::ProcessRemoved { id }) => {
                        // A closed process strands no timers: drop the ones it owned.
                        let _ = self.repo.release_owner(id);
                    }
                    // Every other event, and a lag that dropped some, is only a signal to look
                    // again; the loop re-reads the idle and process registries above, so nothing
                    // is lost by not knowing which event it was.
                    Ok(_) | Err(RecvError::Lagged(_)) => {}
                },
                () = self.wake.notified() => {}
                () = sleep_until_millis(&self.clock, next_deadline) => {}
            }
        }
    }

    /// Whether `timer` should fire now: any timer once its deadline passes (its scheduled time, or
    /// a fire-when-idle backstop), and a fire-when-idle timer as soon as its watched quorum is idle.
    /// The quorum and the per-process idle read are the shared `IdleMode::quorum_met` and
    /// [`watched_process_is_idle`], over the same registries the façade reports from, so this fires
    /// on exactly what its caller was told at set time.
    fn is_due(&self, timer: &StoredTimer, now: u64, supervisor: &Supervisor) -> bool {
        if timer.deadline_unix_millis <= now {
            return true;
        }
        match timer.fire.idle_quorum() {
            None => false,
            Some((mode, watched)) => mode.quorum_met(watched, |p| {
                watched_process_is_idle(self.idle.as_ref(), supervisor, p)
            }),
        }
    }
}

/// Delivers a fired timer's body to its owner as a fresh submitted turn, carrying a compact
/// wake-reason header so the woken agent knows *why* it woke. Best-effort and non-blocking — the
/// timer is already claimed and removed, so an owner that has since gone (or a deaf child whose
/// input channel is full) simply means the body is not delivered; delivery must never stall the
/// loop for every other agent's timers.
fn deliver(supervisor: &Supervisor, timer: StoredTimer, fired_at_backstop: bool) {
    let header = wake_reason_header(&timer, fired_at_backstop);
    let _ = supervisor.try_write_stdin(timer.owner, submitted_turn(&header, &timer.body));
}

/// A compact, clean-room wake-reason header prepended to the delivered body so the woken agent can
/// tell "all peers finished" from "I was timed out" — per-fire context for the lead.
/// Format is `[Soloist timer #<id>] <reason>`.
fn wake_reason_header(timer: &StoredTimer, fired_at_backstop: bool) -> String {
    let id = timer.id;
    match &timer.fire {
        FireCond::At => format!("[Soloist timer #{id}] scheduled delivery"),
        FireCond::WhenIdleAny { watched } => {
            if fired_at_backstop {
                format!(
                    "[Soloist timer #{id}] max-wait backstop elapsed \
                     (when-any-idle, {} watched)",
                    watched.len()
                )
            } else {
                format!("[Soloist timer #{id}] a watched agent is idle (any-idle condition met)")
            }
        }
        FireCond::WhenIdleAll { watched } => {
            if fired_at_backstop {
                format!(
                    "[Soloist timer #{id}] max-wait backstop elapsed \
                     (when-all-idle, {} watched)",
                    watched.len()
                )
            } else {
                format!(
                    "[Soloist timer #{id}] all {} watched agents are idle",
                    watched.len()
                )
            }
        }
    }
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
