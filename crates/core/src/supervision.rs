//! Keeping an internal background task alive (plan/04 §6).
//!
//! The monitoring samplers (metrics, port discovery, and later the file watcher) are
//! long-running loops that must survive a transient fault. [`supervise`] runs such a loop
//! inside a panic-isolation boundary: if it dies, it is restarted after a bounded,
//! exponential backoff driven by the [`Clock`] — so the failure is contained and recovery
//! is deterministic under the mock clock. A loop that returns on its own (its watched
//! resource is gone, e.g. app shutdown) ends the supervision.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::ports::Clock;

/// The first delay before a dead loop is restarted.
const INITIAL_BACKOFF: Duration = Duration::from_millis(200);

/// The ceiling the backoff doubles up to, so a persistently failing loop retries at a
/// steady, bounded cadence rather than ever faster or ever slower.
const MAX_BACKOFF: Duration = Duration::from_secs(5);

/// Runs the loop produced by `make_loop` under panic isolation, restarting it after a
/// backoff if it panics, until it returns on its own (then this returns too). `make_loop`
/// is called once per (re)start and must produce a fresh `'static + Send` future each time
/// — typically `move || self.clone().run_loop()`.
pub(crate) async fn supervise<F, Fut>(clock: Arc<dyn Clock>, mut make_loop: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut backoff = INITIAL_BACKOFF;
    loop {
        match tokio::spawn(make_loop()).await {
            // The loop returned on its own — its work is done (the watched resource is gone).
            Ok(()) => break,
            Err(join_err) if join_err.is_panic() => {
                // Isolate the panic and restart after a bounded backoff, so a transient
                // fault never takes the task down for good.
                clock.sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
            // The task was cancelled (the runtime is shutting down) — stop supervising.
            Err(_) => break,
        }
    }
}

#[cfg(test)]
#[path = "supervision_tests.rs"]
mod tests;
