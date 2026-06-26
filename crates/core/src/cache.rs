//! A read-through cache for derived read-models: compute a value once, then serve the
//! cached copy until it is older than a TTL.
//!
//! It generalizes the memo that the shell-environment resolver and agent auto-detection
//! share — an expensive, off-runtime read (running a shell, probing CLIs) that a burst of
//! callers would otherwise each repeat. One [`Clock`]-driven, single-flighted cache lives
//! here so each consumer declares a TTL instead of re-rolling the mutex/clock/store dance.
//! Caching is policy, not an OS concern, so this is a pure-core utility — only the [`Clock`]
//! is a port — and it holds no business state of its own.

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::ports::Clock;

/// One cached value and when it was computed, per the cache's [`Clock`].
struct Cached<T> {
    value: T,
    at: Instant,
}

/// A read-through cache that serves a computed value for up to a TTL before recomputing.
///
/// The first caller — or the first after the TTL lapses — runs the `compute` closure while
/// holding the lock, so a burst of concurrent callers waits for that single computation
/// rather than each launching its own (single-flight). A successful value is cached for the
/// full TTL; a failed computation is **not** cached, so the next caller retries instead of
/// serving a stale error.
pub(crate) struct ReadCache<T> {
    clock: Arc<dyn Clock>,
    ttl: Duration,
    cached: Mutex<Option<Cached<T>>>,
}

impl<T: Clone> ReadCache<T> {
    /// A cache that reuses a value for `ttl`, timed by `clock`.
    pub(crate) fn new(clock: Arc<dyn Clock>, ttl: Duration) -> Self {
        Self {
            clock,
            ttl,
            cached: Mutex::new(None),
        }
    }

    /// The cached value if it is still within the TTL; otherwise the result of `compute`,
    /// cached on success. The lock is held across `compute` so concurrent callers share one
    /// computation; a failure is propagated without being cached.
    pub(crate) async fn get_or_try_init<E, F, Fut>(&self, compute: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut cached = self.cached.lock().await;
        if let Some(entry) = cached.as_ref() {
            if self.clock.now().saturating_duration_since(entry.at) < self.ttl {
                return Ok(entry.value.clone());
            }
        }
        // Stale or never computed: run `compute` once while holding the lock, so a burst of
        // concurrent callers waits for this one computation instead of each running its own.
        let value = compute().await?;
        *cached = Some(Cached {
            value: value.clone(),
            at: self.clock.now(),
        });
        Ok(value)
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
