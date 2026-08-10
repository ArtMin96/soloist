//! A quiet-window debouncer: collapse a burst of triggers into a single action.
//!
//! A filesystem watcher emits many events for one logical save. The debouncer
//! records each trigger's instant and reports "due" only once the configured quiet
//! window has elapsed with no further trigger — turning a storm of events into one
//! sync. It is a pure state machine driven by [`crate::ports::Clock`]-sourced
//! instants, so it is fully deterministic under the mock clock and adds no timers
//! of its own; the caller decides when to poll it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ports::Clock;

/// Sleeps until `deadline`, or forever when nothing is pending — so a debounce-driven
/// reactor idles without arming a timer whenever no quiet window is in flight.
pub(crate) async fn sleep_until(clock: &Arc<dyn Clock>, deadline: Option<Instant>) {
    match deadline {
        Some(at) => clock.sleep(at.saturating_duration_since(clock.now())).await,
        None => std::future::pending::<()>().await,
    }
}

/// Coalesces triggers within a `quiet` window. Construct one per watched source.
///
/// A quiet window alone has no ceiling: a source whose triggers keep arriving closer together than
/// the window re-arms it every time, and the action is postponed for as long as the triggers last.
/// For sources where that is the right answer — a restart should not fire mid-edit-storm — the
/// window stands alone. Where the action is a *read* of the state everything is changing, it is not:
/// [`Debouncer::bounded`] adds a ceiling on the postponement, so a source that never goes quiet is
/// still acted on, on a bounded schedule.
pub struct Debouncer {
    quiet: Duration,
    /// The longest a burst may postpone its action, or `None` for a window that can be postponed
    /// indefinitely.
    ceiling: Option<Duration>,
    /// When the pending burst began — what the ceiling is measured from, as distinct from the last
    /// trigger the quiet window is measured from.
    burst_began: Option<Instant>,
    last_trigger: Option<Instant>,
    pending: bool,
}

impl Debouncer {
    /// A debouncer with the given quiet window, postponed for as long as its triggers keep coming.
    pub fn new(quiet: Duration) -> Self {
        Self {
            quiet,
            ceiling: None,
            burst_began: None,
            last_trigger: None,
            pending: false,
        }
    }

    /// The same, but where a burst that never goes quiet is acted on anyway once it has been running
    /// for `ceiling` — measured from the burst's first trigger, so a continuously-changing source is
    /// acted on about once per `ceiling` rather than never.
    pub fn bounded(quiet: Duration, ceiling: Duration) -> Self {
        Self {
            ceiling: Some(ceiling),
            ..Self::new(quiet)
        }
    }

    /// Records a trigger at `now`, (re)starting the quiet window. The first trigger of a burst also
    /// starts the ceiling, which later triggers do not restart — that is the whole of what makes it
    /// a ceiling.
    pub fn trigger(&mut self, now: Instant) {
        self.burst_began = self.burst_began.filter(|_| self.pending).or(Some(now));
        self.last_trigger = Some(now);
        self.pending = true;
    }

    /// The instant the pending trigger becomes due — its quiet window elapses, or its burst reaches
    /// the ceiling, whichever is sooner — or `None` when nothing is pending. Lets a caller sleep
    /// exactly until the next action instead of polling on a fixed interval.
    pub fn due_at(&self) -> Option<Instant> {
        let quiet_at = self.last_trigger.filter(|_| self.pending)? + self.quiet;
        match (self.ceiling, self.burst_began) {
            (Some(ceiling), Some(began)) => Some(quiet_at.min(began + ceiling)),
            _ => Some(quiet_at),
        }
    }

    /// Returns `true` exactly once the pending burst is due (see [`Self::due_at`]), then resets
    /// until the next trigger — which begins a fresh burst, and so a fresh ceiling.
    pub fn take_if_due(&mut self, now: Instant) -> bool {
        match self.due_at() {
            Some(due) if now >= due => {
                self.pending = false;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "debounce_tests.rs"]
mod tests;
