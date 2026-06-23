//! A manually-advanced [`Clock`] fake. Time only moves when a test calls
//! [`MockClock::advance`], so the grace window and every timer-driven path run
//! deterministically with no real time elapsed.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::ports::Clock;
use crate::sync::lock;

struct Sleeper {
    deadline: Instant,
    waker: oneshot::Sender<()>,
}

/// A fixed wall-clock starting point so [`MockClock::now_unix_millis`] returns a realistic
/// absolute time (well after 1970) that a test can reason about. Arbitrary; only the deltas
/// produced by [`MockClock::advance`] matter to the durable time-based paths.
const MOCK_WALL_EPOCH_MILLIS: u64 = 1_700_000_000_000;

struct MockState {
    now: Instant,
    /// Wall-clock millis, advanced in lockstep with `now` so a persisted deadline behaves the
    /// same as the monotonic clock the rest of the mock drives.
    wall_millis: u64,
    sleepers: Vec<Sleeper>,
}

/// A [`Clock`] whose time only moves when the test calls [`MockClock::advance`].
/// `sleep` registers a waiter that completes once time passes its deadline.
#[derive(Clone)]
pub struct MockClock {
    state: Arc<Mutex<MockState>>,
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                now: Instant::now(),
                wall_millis: MOCK_WALL_EPOCH_MILLIS,
                sleepers: Vec::new(),
            })),
        }
    }

    /// Advances time by `by`, completing every sleeper whose deadline has passed.
    pub fn advance(&self, by: Duration) {
        let mut state = lock(&self.state);
        state.now += by;
        state.wall_millis += by.as_millis() as u64;
        let now = state.now;
        let mut pending = Vec::new();
        for sleeper in state.sleepers.drain(..) {
            if sleeper.deadline <= now {
                let _ = sleeper.waker.send(());
            } else {
                pending.push(sleeper);
            }
        }
        state.sleepers = pending;
    }
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Clock for MockClock {
    fn now(&self) -> Instant {
        lock(&self.state).now
    }

    fn now_unix_millis(&self) -> u64 {
        lock(&self.state).wall_millis
    }

    async fn sleep(&self, dur: Duration) {
        let rx = {
            let mut state = lock(&self.state);
            let deadline = state.now + dur;
            if deadline <= state.now {
                return;
            }
            let (tx, rx) = oneshot::channel();
            state.sleepers.push(Sleeper {
                deadline,
                waker: tx,
            });
            rx
        };
        let _ = rx.await;
    }
}
