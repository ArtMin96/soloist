//! What a run somebody is watching reports, how often, and what watching does *not* change.
//!
//! Every test here bounds its own wait: the failure a missing report causes is a run that never
//! finishes, and a run that never finishes must fail the test rather than hang it.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::*;

/// Long enough that a report certainly has time to arrive and be acted on, short enough that a run
/// which never reports fails the test in seconds rather than minutes.
const PATIENCE: Duration = Duration::from_secs(10);

/// Generous enough that no test here reaches it.
const ROOMY: usize = 64 * 1024;

/// The account a watched run carries back is capped at this, exactly as an unwatched one's is.
const ACCOUNT: usize = 256;

/// One run, watched by `observer` at `interval`.
fn watched<'a>(observer: &'a dyn Fn(&str), interval: Duration) -> Run<'a> {
    Run {
        input: None,
        stopped: None,
        time_limit: PATIENCE,
        watching: Some(Watch { interval, observer }),
        output_limit: ROOMY,
        diagnostics: Some(ACCOUNT),
    }
}

fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

/// Everything an observer was told, in order.
fn heard() -> (Arc<Mutex<Vec<String>>>, impl Fn(&str) + use<>) {
    let said = Arc::new(Mutex::new(Vec::new()));
    let collecting = Arc::clone(&said);
    (said, move |remark: &str| {
        collecting
            .lock()
            .expect("nothing panics holding this")
            .push(remark.to_string())
    })
}

#[test]
fn a_watched_run_is_heard_from_while_it_is_still_running_rather_than_once_it_has_ended() {
    // The run cannot end until its first remark has been heard, so hearing it only at the end would
    // deadlock — and the time limit turns that deadlock into a failed assertion rather than a hang.
    let dir = tempfile::tempdir().expect("temp dir");
    let gate = dir.path().join("heard");
    let opened = gate.clone();
    let observer = move |_remark: &str| {
        let _ = std::fs::write(&opened, "");
    };

    let finished = run(
        shell(&format!(
            "printf 'Writing objects:  10%%\\r' >&2; \
             while [ ! -f {} ]; do sleep 0.02; done; \
             printf done",
            gate.display()
        )),
        watched(&observer, Duration::ZERO),
    )
    .expect("the run ends, which it can only do once its first remark has been heard");

    assert_eq!(String::from_utf8_lossy(&finished.output), "done");
}

#[test]
fn what_a_watched_run_says_reaches_its_watcher_as_the_program_wrote_it() {
    let (said, observer) = heard();

    run(
        // A carriage return is how version control ends a progress remark and a newline is how it
        // ends a finished one, so both end one here. They are spaced because a remark superseded
        // before anyone reads it is dropped by design — that is what the coalescing *is* — and this
        // test is about the wording that arrives rather than about how much of it does.
        shell(
            "printf 'Counting objects: 100%%\\r' >&2; sleep 0.2; \
             printf 'Writing objects: 100%%\\n' >&2; sleep 0.2",
        ),
        watched(&observer, Duration::ZERO),
    )
    .expect("finished");

    let said = said.lock().expect("nothing panics holding this");
    assert_eq!(
        *said,
        vec!["Counting objects: 100%", "Writing objects: 100%"],
        "the watcher hears what the program actually wrote, not a summary of it",
    );
}

#[test]
fn a_run_that_will_not_stop_talking_is_reported_on_at_the_interval_rather_than_per_remark() {
    let told = Arc::new(AtomicUsize::new(0));
    let counting = Arc::clone(&told);
    let observer = move |_remark: &str| {
        counting.fetch_add(1, Ordering::SeqCst);
    };
    let interval = Duration::from_millis(200);

    let started = Instant::now();
    run(
        // Remarks written as fast as a shell can write them, for long enough that the interval is
        // what bounds the reporting rather than how briefly the run lasted. A run measured in
        // milliseconds would be bounded by its own brevity and would prove nothing.
        shell(
            "end=$(( $(date +%s) + 1 )); i=0; \
             while [ $(date +%s) -lt $end ]; do printf 'at %s\\r' $i >&2; i=$((i+1)); done",
        ),
        watched(&observer, interval),
    )
    .expect("finished");

    // One report per window it ran for, plus the first — which is reported at once rather than
    // waited for, so that a long operation is heard from immediately.
    let windows = started.elapsed().as_millis() / interval.as_millis() + 2;
    let told = told.load(Ordering::SeqCst) as u128;
    assert!(
        told <= windows,
        "a run of {:?} became {told} reports, which is more than the {windows} the interval allows",
        started.elapsed(),
    );
}

#[test]
fn a_watched_run_that_is_asked_to_stop_stops_as_promptly_as_an_unwatched_one() {
    let asked = Arc::new(AtomicBool::new(false));
    // Stopping is asked for the moment the run is first heard from, so the stop lands squarely in
    // the middle of it reporting.
    let stopping = Arc::clone(&asked);
    let observer = move |_remark: &str| stopping.store(true, Ordering::SeqCst);
    let stopped = {
        let asked = Arc::clone(&asked);
        move || asked.load(Ordering::SeqCst)
    };

    let started = Instant::now();
    let outcome = run(
        shell("while true; do printf 'still going\\r' >&2; sleep 0.02; done"),
        Run {
            stopped: Some(&stopped),
            ..watched(&observer, Duration::ZERO)
        },
    );

    assert_eq!(outcome.err(), Some(RunError::Stopped));
    assert!(
        started.elapsed() < PATIENCE / 2,
        "a stop mid-report was waited out rather than acted on: {:?}",
        started.elapsed(),
    );
}

#[test]
fn watching_a_run_does_not_widen_the_account_it_carries_back() {
    let (_said, observer) = heard();
    let chatty =
        "i=0; while [ $i -lt 400 ]; do printf 'a line about itself %s\\n' $i >&2; i=$((i+1)); done";

    let watched_run = run(shell(chatty), watched(&observer, Duration::ZERO)).expect("finished");
    let unwatched = run(
        shell(chatty),
        Run {
            watching: None,
            ..watched(&observer, Duration::ZERO)
        },
    )
    .expect("finished");

    assert!(
        watched_run.diagnostics.len() <= ACCOUNT,
        "watching grew the account past its ceiling: {} bytes",
        watched_run.diagnostics.len(),
    );
    assert_eq!(
        watched_run.diagnostics, unwatched.diagnostics,
        "watching changed what the run carried back",
    );
}
