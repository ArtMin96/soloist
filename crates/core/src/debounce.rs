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
pub struct Debouncer {
    quiet: Duration,
    last_trigger: Option<Instant>,
    pending: bool,
}

impl Debouncer {
    /// A debouncer with the given quiet window.
    pub fn new(quiet: Duration) -> Self {
        Self {
            quiet,
            last_trigger: None,
            pending: false,
        }
    }

    /// Records a trigger at `now`, (re)starting the quiet window.
    pub fn trigger(&mut self, now: Instant) {
        self.last_trigger = Some(now);
        self.pending = true;
    }

    /// The instant the pending trigger becomes due (its quiet window elapses), or `None`
    /// when nothing is pending. Lets a caller sleep exactly until the next action instead of
    /// polling on a fixed interval.
    pub fn due_at(&self) -> Option<Instant> {
        self.last_trigger
            .filter(|_| self.pending)
            .map(|last| last + self.quiet)
    }

    /// Returns `true` exactly once after the quiet window elapses since the last
    /// trigger with at least one trigger pending, then resets until the next
    /// trigger. `saturating_duration_since` keeps a non-monotonic instant from
    /// panicking.
    pub fn take_if_due(&mut self, now: Instant) -> bool {
        match self.last_trigger {
            Some(last) if self.pending && now.saturating_duration_since(last) >= self.quiet => {
                self.pending = false;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Clock;
    use crate::testing::MockClock;

    #[test]
    fn fires_once_after_the_quiet_window_and_resets() {
        let clock = MockClock::new();
        let mut debouncer = Debouncer::new(Duration::from_millis(500));

        debouncer.trigger(clock.now());
        assert!(!debouncer.take_if_due(clock.now()), "not due immediately");

        clock.advance(Duration::from_millis(499));
        assert!(
            !debouncer.take_if_due(clock.now()),
            "not due before the window"
        );

        clock.advance(Duration::from_millis(1));
        assert!(debouncer.take_if_due(clock.now()), "due at the window");
        assert!(
            !debouncer.take_if_due(clock.now()),
            "fires only once per burst"
        );
    }

    #[test]
    fn a_later_trigger_restarts_the_window() {
        let clock = MockClock::new();
        let mut debouncer = Debouncer::new(Duration::from_millis(500));

        debouncer.trigger(clock.now());
        clock.advance(Duration::from_millis(300));
        debouncer.trigger(clock.now()); // resets the window
        clock.advance(Duration::from_millis(300)); // 300 < 500 since the last trigger
        assert!(!debouncer.take_if_due(clock.now()));
        clock.advance(Duration::from_millis(200)); // now 500 since the last trigger
        assert!(debouncer.take_if_due(clock.now()));
    }
}
