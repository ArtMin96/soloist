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

struct MockState {
    now: Instant,
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
                sleepers: Vec::new(),
            })),
        }
    }

    /// Advances time by `by`, completing every sleeper whose deadline has passed.
    pub fn advance(&self, by: Duration) {
        let mut state = lock(&self.state);
        state.now += by;
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
