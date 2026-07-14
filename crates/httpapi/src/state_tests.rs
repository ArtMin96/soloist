use std::time::Duration;

use super::{SpawnRateLimiter, SPAWN_MAX_PER_WINDOW, SPAWN_WINDOW};

#[test]
fn the_limiter_admits_up_to_the_cap_then_refuses_within_a_window() {
    let base = std::time::Instant::now();
    let limiter = SpawnRateLimiter::new(base);
    for _ in 0..SPAWN_MAX_PER_WINDOW {
        assert!(limiter.check(base), "each spawn up to the cap is admitted");
    }
    assert!(
        !limiter.check(base),
        "the spawn past the cap is refused within the window"
    );
}

#[test]
fn the_limiter_rolls_the_window_over_and_admits_again() {
    let base = std::time::Instant::now();
    let limiter = SpawnRateLimiter::new(base);
    for _ in 0..SPAWN_MAX_PER_WINDOW {
        assert!(limiter.check(base));
    }
    assert!(!limiter.check(base), "capped within the window");

    // Once the window elapses, the count resets and spawns are admitted again.
    let next_window = base + SPAWN_WINDOW + Duration::from_millis(1);
    assert!(
        limiter.check(next_window),
        "a spawn after the window resets the count"
    );
}
