//! Behavioural tests for [`supervise`], kept out of the implementation file. They drive
//! the mock clock so the restart backoff elapses with no real time.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::testing::MockClock;

use super::{supervise, MAX_BACKOFF};

#[tokio::test]
async fn a_panicking_loop_is_restarted_until_it_completes() {
    // The loop panics on its first two runs, then returns — supervision must restart it
    // each time and stop once it completes, so it ran exactly three times.
    let runs = Arc::new(AtomicUsize::new(0));
    let clock = MockClock::new();

    let task_runs = runs.clone();
    let supervised = tokio::spawn(supervise(Arc::new(clock.clone()), move || {
        let runs = task_runs.clone();
        async move {
            let attempt = runs.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                #[allow(clippy::panic)]
                {
                    panic!("loop {attempt} fails");
                }
            }
            // The third run completes normally.
        }
    }));

    // Drive the backoff sleeps until supervision returns (the loop completed).
    for _ in 0..50 {
        if supervised.is_finished() {
            break;
        }
        clock.advance(MAX_BACKOFF);
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }

    supervised.await.expect("supervision task joins cleanly");
    assert_eq!(
        runs.load(Ordering::SeqCst),
        3,
        "panicked twice, then completed"
    );
}

#[tokio::test]
async fn a_clean_loop_runs_once_and_stops() {
    // A loop that returns immediately is not restarted.
    let runs = Arc::new(AtomicUsize::new(0));
    let clock = MockClock::new();
    let task_runs = runs.clone();
    supervise(Arc::new(clock), move || {
        let runs = task_runs.clone();
        async move {
            runs.fetch_add(1, Ordering::SeqCst);
        }
    })
    .await;
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}
