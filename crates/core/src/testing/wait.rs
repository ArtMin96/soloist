//! A ceiling on the waits the test fakes perform, so a wait that will never be satisfied fails
//! loudly instead of parking for ever.
//!
//! An unbounded `await` is the worst shape a test failure can take: it consumes the whole CI job
//! timeout and reports nothing, so a missed wakeup reads as an infrastructure stall rather than
//! the bug it is. Every waiter in [`crate::testing`] goes through this, so the ceiling is one
//! value in one place.

use std::future::Future;
use std::time::Duration;

/// The longest a waiter blocks before it gives up. Far above what these waits actually take —
/// they observe effects driven by [`MockClock`](super::MockClock), which complete in
/// microseconds — and far below a CI job's timeout, so it fires only on a wait that was never
/// going to be satisfied.
const LIMIT: Duration = Duration::from_secs(30);

/// Awaits `future`, panicking with `what` if it has not resolved within [`LIMIT`].
#[allow(clippy::panic)]
pub(crate) async fn bounded<F: Future>(what: &str, future: F) -> F::Output {
    match tokio::time::timeout(LIMIT, future).await {
        Ok(resolved) => resolved,
        Err(_) => panic!("timed out after {LIMIT:?} waiting for {what}"),
    }
}
