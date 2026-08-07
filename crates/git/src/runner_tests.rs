//! Which exit statuses this adapter reads as an answer. Not a path a repository can be built to
//! provoke, so it is driven directly. The containment around an invocation — the time limit, the
//! output ceiling, the kill and reap — belongs to `soloist_exec` and is tested there.

use std::os::unix::process::ExitStatusExt;

use super::*;

/// A status as the operating system reports one: `code` shifted into place, or a bare signal
/// number for a process that was stopped rather than exiting.
fn exited(code: i32) -> ExitStatus {
    ExitStatus::from_raw(code << 8)
}

fn killed_by(signal: i32) -> ExitStatus {
    ExitStatus::from_raw(signal)
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

#[test]
fn a_missing_tool_is_told_apart_from_an_invocation_that_failed() {
    // The distinction the whole read path degrades on: no `git` at all is reported as such, where
    // any other way of not finishing is an ordinary failure.
    assert_eq!(
        failure(RunError::Spawn(io::ErrorKind::NotFound)),
        GitError::GitMissing
    );
    assert_eq!(
        failure(RunError::Spawn(io::ErrorKind::PermissionDenied)),
        GitError::Op { status: None }
    );
    assert_eq!(failure(RunError::TimedOut), GitError::Timeout);
}
