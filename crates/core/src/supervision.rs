//! Keeping an internal background task alive.
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

/// Runs a blocking OS read on the blocking thread pool, so a periodic sampler never stalls
/// a runtime worker on a `/proc` or `sysinfo` sweep. A panic inside the read is re-raised
/// on the caller (the supervised loop), so [`supervise`] isolates and restarts it exactly
/// as for any other loop panic.
pub(crate) async fn run_blocking<T, F>(read: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    match tokio::task::spawn_blocking(read).await {
        Ok(value) => value,
        // A blocking task cannot be cancelled, so a join error is always a panic; re-raise
        // it here so the supervised loop's panic-isolation boundary catches and restarts it.
        Err(join_err) => std::panic::resume_unwind(join_err.into_panic()),
    }
}

#[cfg(test)]
#[path = "supervision_tests.rs"]
mod tests;
