//! What happens to a run that will not finish, one that will not stop talking, and one whose
//! subject arrives on its standard input. None is a path a caller can be built to provoke, so all
//! are driven directly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nix::errno::Errno;
use nix::sys::signal::kill;

use super::*;

/// Long enough that the process is certainly still running when the limit expires.
const FOREVER: &str = "600";

/// Short enough to keep the test quick, long enough not to expire before the child has started.
const LIMIT: Duration = Duration::from_millis(200);

/// Generous enough that no test here reaches it except the one about reaching it.
const ROOMY: usize = 64 * 1024;

fn bounded(input: Option<&str>) -> Run<'_> {
    Run {
        input,
        stopped: None,
        time_limit: Duration::from_secs(10),
        watching: None,
        output_limit: ROOMY,
        diagnostics: None,
    }
}

fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

/// The limit the stop test runs under — far longer than the child it stops, so running out of time
/// is never what ends that run.
const STOPPED_WELL_WITHIN: Duration = Duration::from_secs(30);

/// How long the stopped child would live untouched, and how long the run may take once it has been
/// asked to stop. A run that waited for the child instead of killing it takes the first, so the
/// second is what tells the two apart.
const CHILD_LIFE: &str = "sleep 10";
const ENDS_WITHIN: Duration = Duration::from_secs(3);

/// How long the test itself waits for the bounded wait to answer. Comfortably past [`LIMIT`], and
/// present at all because the wait's bound is the very thing under test: a run that stopped killing
/// its child would otherwise leave this test waiting out [`FOREVER`] rather than failing.
const GRACE: Duration = Duration::from_secs(10);

#[test]
fn a_run_that_will_not_finish_is_stopped_and_reaped() {
    let child = Command::new("sleep")
        .arg(FOREVER)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("spawn");
    let group = Pid::from_raw(child.id() as i32);

    let (answered, waiting) = mpsc::channel();
    thread::spawn(move || {
        let _ = answered.send(
            wait_bounded(
                child,
                &Run {
                    time_limit: LIMIT,
                    ..bounded(None)
                },
                None,
                None,
            )
            .is_err_and(|err| err == RunError::TimedOut),
        );
    });
    let timed_out = waiting.recv_timeout(GRACE);

    if timed_out.is_err() {
        // The wait never came back, so it is holding the child rather than stopping it. Clean up
        // what the run should have, then say what happened.
        let _ = killpg(group, Signal::SIGKILL);
        panic!("the wait never returned, so nothing bounded the run it was supposed to bound");
    }
    assert_eq!(
        timed_out,
        Ok(true),
        "a run past its limit has to be reported as out of time, not as some other failure",
    );
    assert_eq!(
        kill(group, None),
        Err(Errno::ESRCH),
        "the stopped process was reaped, so nothing of it is left — not even a zombie",
    );
}

#[test]
fn a_run_somebody_changed_their_mind_about_is_stopped_and_reaped_like_one_out_of_time() {
    let asked_to_stop = Arc::new(AtomicBool::new(false));
    let stopped = {
        let asked_to_stop = Arc::clone(&asked_to_stop);
        move || asked_to_stop.load(Ordering::SeqCst)
    };
    // Asked to stop from another thread once the run is certainly under way, which is the only way
    // it happens for real.
    let asking = {
        let asked_to_stop = Arc::clone(&asked_to_stop);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            asked_to_stop.store(true, Ordering::SeqCst);
        })
    };

    let started = std::time::Instant::now();
    let outcome = run(
        shell(CHILD_LIFE),
        Run {
            stopped: Some(&stopped),
            // Bounded well past the moment the stop arrives, but bounded: a run that ignored the
            // signal has to be reported rather than waited out to a limit nobody would sit through.
            time_limit: STOPPED_WELL_WITHIN,
            ..bounded(None)
        },
    );
    let took = started.elapsed();
    asking.join().expect("the asking thread");

    assert_eq!(
        outcome.err(),
        Some(RunError::Stopped),
        "being asked to stop is not running out of time and not a failure — it is what was asked",
    );
    assert!(
        took < ENDS_WITHIN,
        "and it ends the run then and there rather than waiting for the child to finish on its \
         own: took {took:?}",
    );
}

#[test]
fn a_run_that_was_stopped_before_it_started_never_starts_anything() {
    // A program that could not be started at all: reaching the spawn reports that it is missing, so
    // reporting the stop instead is proof nothing was reached.
    let outcome = run(
        Command::new("soloist-no-such-program"),
        Run {
            stopped: Some(&|| true),
            ..bounded(None)
        },
    );

    assert_eq!(
        outcome.err(),
        Some(RunError::Stopped),
        "a run stopped before it began costs no process at all, not even a failed spawn",
    );
}

#[test]
fn a_run_nobody_stopped_still_answers_normally() {
    let finished = run(
        shell("printf answered"),
        Run {
            stopped: Some(&|| false),
            ..bounded(None)
        },
    )
    .expect("finished");

    assert_eq!(String::from_utf8_lossy(&finished.output), "answered");
    assert!(finished.status.success());
}

#[test]
fn a_run_past_its_output_ceiling_is_stopped_rather_than_read_to_the_end() {
    let outcome = run(
        shell("yes soloist"),
        Run {
            output_limit: 4 * 1024,
            ..bounded(None)
        },
    );

    assert!(
        matches!(outcome, Err(RunError::OverLimit { .. })),
        "an endless writer is refused at the ceiling, not followed until memory runs out",
    );
}

#[test]
fn a_run_past_the_ceiling_is_stopped_there_rather_than_held_to_its_time_limit() {
    // A writer that dies when nobody is reading it — `yes` above — would be caught by the ceiling
    // either way. One that writes too much and then carries on would not: without being stopped at
    // the ceiling it holds the run until the time limit and reports having run out of time, which
    // says the wrong thing about a tool that answered far too much far too quickly.
    let outcome = run(
        shell("yes soloist | head -c 100000; sleep 600"),
        Run {
            time_limit: Duration::from_secs(30),
            output_limit: 4 * 1024,
            ..bounded(None)
        },
    );

    assert!(
        matches!(outcome, Err(RunError::OverLimit { .. })),
        "past the ceiling is what happened, and it is what has to be reported: {outcome:?}",
    );
}

#[test]
fn a_run_reads_its_subject_from_standard_input() {
    let finished = run(shell("cat"), bounded(Some("the whole subject"))).expect("finished");

    assert_eq!(
        String::from_utf8_lossy(&finished.output),
        "the whole subject"
    );
    assert!(finished.status.success());
}

#[test]
fn what_a_failing_run_wrote_about_itself_is_carried_back_only_when_asked_for() {
    let asked = run(
        shell("echo refused >&2; exit 3"),
        Run {
            diagnostics: Some(ROOMY),
            ..bounded(None)
        },
    )
    .expect("finished");
    assert_eq!(asked.diagnostics, "refused");
    assert_eq!(asked.status.code(), Some(3));

    let unasked = run(shell("echo refused >&2; exit 3"), bounded(None)).expect("finished");
    assert_eq!(
        unasked.diagnostics, "",
        "prose nobody asked to show is discarded rather than collected",
    );
}

#[test]
fn a_run_nobody_asked_diagnostics_of_can_still_write_as_much_as_it_likes() {
    // Prose nobody collects has to go nowhere rather than into a pipe nobody drains: a command
    // writing more about itself than a pipe holds would block on the write and be stopped at the
    // time limit, having finished its actual work.
    let finished = run(
        shell("yes 'a warning nobody asked for' | head -c 200000 >&2; printf done"),
        bounded(None),
    )
    .expect("finished");

    assert_eq!(String::from_utf8_lossy(&finished.output), "done");
    assert!(finished.status.success());
}

#[test]
fn a_program_that_is_not_installed_is_told_apart_from_one_that_failed() {
    let outcome = run(Command::new("soloist-no-such-program"), bounded(None));

    assert_eq!(
        outcome.err(),
        Some(RunError::Spawn(io::ErrorKind::NotFound))
    );
}
