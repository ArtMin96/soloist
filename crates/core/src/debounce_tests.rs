//! Behavioural tests for the quiet-window debouncer, driven on the mock clock so no real time
//! elapses.

use std::time::Duration;

use super::Debouncer;
use crate::ports::Clock;
use crate::testing::MockClock;

/// The window under test, long enough that the steps below sit clearly inside or outside it.
const QUIET: Duration = Duration::from_millis(500);

/// The longest a bounded burst may postpone its action.
const CEILING: Duration = Duration::from_secs(2);

/// A gap shorter than [`QUIET`] — a stream of triggers this close together never lets the window
/// elapse.
const RESTLESS: Duration = Duration::from_millis(400);

/// Enough restless steps to run well past [`CEILING`], so a ceiling that holds is a ceiling that
/// fired rather than one that merely had not been reached.
const STREAM_STEPS: usize = 20;

#[test]
fn fires_once_after_the_quiet_window_and_resets() {
    let clock = MockClock::new();
    let mut debouncer = Debouncer::new(QUIET);

    debouncer.trigger(clock.now());
    assert!(!debouncer.take_if_due(clock.now()), "not due immediately");

    clock.advance(QUIET - Duration::from_millis(1));
    assert!(
        !debouncer.take_if_due(clock.now()),
        "not due before the window"
    );

    clock.advance(Duration::from_millis(1));
    assert!(debouncer.take_if_due(clock.now()), "due at the window");
    assert!(
        !debouncer.take_if_due(clock.now()),
        "fires only once per burst"
    );
}

#[test]
fn a_later_trigger_restarts_the_window() {
    let clock = MockClock::new();
    let mut debouncer = Debouncer::new(QUIET);

    debouncer.trigger(clock.now());
    clock.advance(Duration::from_millis(300));
    debouncer.trigger(clock.now()); // resets the window
    clock.advance(Duration::from_millis(300)); // 300 < 500 since the last trigger
    assert!(!debouncer.take_if_due(clock.now()));
    clock.advance(Duration::from_millis(200)); // now 500 since the last trigger
    assert!(debouncer.take_if_due(clock.now()));
}

#[test]
fn a_burst_that_never_goes_quiet_is_postponed_for_ever_without_a_ceiling() {
    let clock = MockClock::new();
    let mut debouncer = Debouncer::new(QUIET);

    // What an agent writing file after file looks like: every trigger lands inside the window the
    // one before it opened, so the window never elapses and the action never comes.
    for _ in 0..STREAM_STEPS {
        debouncer.trigger(clock.now());
        clock.advance(RESTLESS);
        assert!(
            !debouncer.take_if_due(clock.now()),
            "an unbounded window is postponed by every trigger, however long the stream runs",
        );
    }
}

#[test]
fn a_ceiling_acts_on_a_burst_that_never_goes_quiet() {
    let clock = MockClock::new();
    let mut debouncer = Debouncer::bounded(QUIET, CEILING);
    let began = clock.now();

    let fired_at = (0..STREAM_STEPS)
        .find_map(|_| {
            debouncer.trigger(clock.now());
            clock.advance(RESTLESS);
            debouncer.take_if_due(clock.now()).then(|| clock.now())
        })
        .expect("the ceiling acted on a burst the quiet window would have postponed for ever");

    // At the ceiling, not at some later point a longer stream happened to reach.
    assert!(
        fired_at.duration_since(began) < CEILING + RESTLESS,
        "it waited past the ceiling: {:?}",
        fired_at.duration_since(began),
    );
}

#[test]
fn a_ceiling_leaves_a_burst_that_does_go_quiet_to_its_quiet_window() {
    let clock = MockClock::new();
    let mut debouncer = Debouncer::bounded(QUIET, CEILING);

    // One trigger, then silence: the quiet window is what decides, exactly as without a ceiling.
    debouncer.trigger(clock.now());
    clock.advance(QUIET - Duration::from_millis(1));
    assert!(
        !debouncer.take_if_due(clock.now()),
        "a ceiling does not shorten the quiet window"
    );
    clock.advance(Duration::from_millis(1));
    assert!(debouncer.take_if_due(clock.now()));
}

#[test]
fn each_burst_gets_the_whole_ceiling_again() {
    let clock = MockClock::new();
    let mut debouncer = Debouncer::bounded(QUIET, CEILING);

    // A burst that ran up to the ceiling and fired.
    debouncer.trigger(clock.now());
    clock.advance(CEILING);
    assert!(debouncer.take_if_due(clock.now()));

    // The next trigger begins a fresh burst, so it is not already overdue on the old one's clock.
    debouncer.trigger(clock.now());
    assert!(
        !debouncer.take_if_due(clock.now()),
        "the ceiling carried over from the burst before, firing with no quiet window at all",
    );
}
