//! That a run somebody stopped ends at once and leaves nothing of itself behind.
//!
//! The question is about *this process's* children, so it can only be asked where there is exactly
//! one run to ask about — hence a binary of its own holding one test. Another test's child, alive in
//! the same binary, would answer for itself instead.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;

use soloist_exec::{run, Run, RunError};

/// How long the child would live if nothing stopped it. Long enough that returning before it is a
/// fact about the stop rather than about timing, short enough that a test which fails to stop it
/// fails in seconds.
const CHILD_LIFE: &str = "sleep 10";

/// Longer than the child's own life, so running out of time is never what ends this run.
const UNREACHED: Duration = Duration::from_secs(30);

/// When the stop is asked for, and how long after it the run may still take. Well clear of the
/// child's own life, so a run that waited for the child instead of killing it is reported.
const ASKED_AFTER: Duration = Duration::from_millis(100);
const ENDS_WITHIN: Duration = Duration::from_secs(3);

#[test]
fn a_stopped_run_ends_at_once_and_leaves_neither_a_child_nor_a_zombie() {
    let asked_to_stop = Arc::new(AtomicBool::new(false));
    let stopped = {
        let asked_to_stop = Arc::clone(&asked_to_stop);
        move || asked_to_stop.load(Ordering::SeqCst)
    };
    let asking = {
        let asked_to_stop = Arc::clone(&asked_to_stop);
        thread::spawn(move || {
            thread::sleep(ASKED_AFTER);
            asked_to_stop.store(true, Ordering::SeqCst);
        })
    };
    let mut command = std::process::Command::new("sh");
    command.arg("-c").arg(CHILD_LIFE);
    let started = Instant::now();

    let outcome = run(
        command,
        Run {
            input: None,
            stopped: Some(&stopped),
            time_limit: UNREACHED,
            output_limit: 64 * 1024,
            diagnostics: None,
        },
    );
    let took = started.elapsed();
    asking.join().expect("the asking thread");

    assert_eq!(outcome.err(), Some(RunError::Stopped));
    assert!(
        took < ENDS_WITHIN,
        "stopping has to end the run rather than wait for the child to finish on its own: took \
         {took:?}",
    );
    assert_eq!(
        waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)),
        Err(Errno::ECHILD),
        "and it reaps it, so no child of this process is left alive and none is a zombie",
    );
}
