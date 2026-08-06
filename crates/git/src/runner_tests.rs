//! What happens to an invocation that will not finish, and to one whose exit status has to be
//! read. Neither is a path a repository can be built to provoke, so both are driven directly.

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

use nix::errno::Errno;
use nix::sys::signal::kill;

use super::*;

/// A status as the operating system reports one: `code` shifted into place, or a bare signal
/// number for a process that was stopped rather than exiting.
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

fn killed_by(signal: i32) -> ExitStatus {
    ExitStatus::from_raw(signal)
}

/// Long enough that the process is certainly still running when the limit expires.
const FOREVER: &str = "600";

/// Short enough to keep the test quick, long enough not to expire before the child has started.
const LIMIT: Duration = Duration::from_millis(200);

#[test]
fn an_invocation_that_will_not_finish_is_stopped_and_reaped() {
    let child = Command::new("sleep")
        .arg(FOREVER)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("spawn");
    let pid = Pid::from_raw(child.id() as i32);

    let outcome = wait_bounded(child, LIMIT);

    assert!(
        matches!(outcome, Err(GitError::Timeout)),
        "expected a timeout, got {outcome:?}",
    );
    assert_eq!(
        kill(pid, None),
        Err(Errno::ESRCH),
        "the stopped process was reaped, so nothing of it is left — not even a zombie",
    );
}

#[test]
fn an_invocation_that_succeeded_answered_whatever_else_its_caller_would_accept() {
    assert!(answered(exited(0), None));
    assert!(answered(exited(0), Some(1)));
}

#[test]
fn an_invocation_that_exited_with_the_accepted_status_answered() {
    assert!(answered(exited(1), Some(1)));
    assert!(!answered(exited(1), Some(2)));
    assert!(!answered(exited(1), None));
}

#[test]
fn an_invocation_stopped_by_a_signal_never_answered() {
    // It reports no status at all — the same "no status" a caller that accepts nothing carries,
    // which is exactly why the two must not be compared to each other.
    assert!(
        !answered(killed_by(9), None),
        "a killed invocation produced no answer, whatever it managed to write first",
    );
    assert!(!answered(killed_by(9), Some(1)));
}
