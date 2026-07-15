use std::panic::{self, AssertUnwindSafe};
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

/// A caller that panics while holding the window lock poisons it. The limiter serves a long-running
/// server, so it must take the window back and keep deciding — a poisoning that refused every later
/// spawn for the life of the process would turn one panic into a permanent outage.
#[test]
fn the_limiter_keeps_admitting_after_its_window_lock_is_poisoned() {
    let base = std::time::Instant::now();
    let limiter = SpawnRateLimiter::new(base);

    // Poison the lock the way a panicking caller would: unwind while holding the guard. The hook is
    // silenced first so the deliberate panic does not print a scary trace in a passing run.
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let panicked = panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = limiter
            .window
            .lock()
            .expect("the fresh lock is uncontended");
        panic!("a caller panicked while holding the window");
    }));
    panic::set_hook(hook);

    assert!(panicked.is_err(), "the deliberate panic unwound");
    assert!(
        limiter.window.is_poisoned(),
        "the panic left the window lock poisoned"
    );
    assert!(
        limiter.check(base),
        "the limiter recovers the poisoned window and still admits"
    );
}
