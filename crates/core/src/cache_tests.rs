//! Unit tests for the read-through cache, on a mock [`Clock`]: a value is computed once and
//! reused within the TTL, recomputed after it, and a failed computation is not cached.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::ReadCache;
use crate::testing::MockClock;

/// A TTL long enough that two back-to-back calls land inside it.
const TTL: Duration = Duration::from_secs(60);

#[tokio::test]
async fn value_is_computed_once_and_reused_within_the_ttl() {
    let calls = AtomicUsize::new(0);
    let cache: ReadCache<u32> = ReadCache::new(Arc::new(MockClock::new()), TTL);

    for _ in 0..2 {
        let value = cache
            .get_or_try_init(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<u32, ()>(7)
            })
            .await;
        assert_eq!(value, Ok(7));
    }

    // Both calls within the window shared the one computation.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn value_is_recomputed_after_the_ttl() {
    let calls = AtomicUsize::new(0);
    let clock = MockClock::new();
    let cache: ReadCache<u32> = ReadCache::new(Arc::new(clock.clone()), TTL);

    let compute = || async {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok::<u32, ()>(1)
    };
    cache.get_or_try_init(compute).await.expect("first compute");

    // Past the TTL the next caller recomputes rather than serving the stale value.
    clock.advance(TTL + Duration::from_secs(1));
    cache
        .get_or_try_init(compute)
        .await
        .expect("second compute");

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_failed_computation_is_not_cached() {
    let cache: ReadCache<u32> = ReadCache::new(Arc::new(MockClock::new()), TTL);

    let first = cache
        .get_or_try_init(|| async { Err::<u32, &str>("boom") })
        .await;
    assert_eq!(first, Err("boom"));

    // The failure was not cached, so the next caller computes again and can succeed.
    let calls = AtomicUsize::new(0);
    let second = cache
        .get_or_try_init(|| async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<u32, &str>(42)
        })
        .await;
    assert_eq!(second, Ok(42));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
