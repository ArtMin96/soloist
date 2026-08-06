//! What happens to an invocation that will not finish. It is the one path no repository can be
//! built to provoke, so it is driven directly with a process that simply waits.

use std::process::{Command, Stdio};

use nix::errno::Errno;
use nix::sys::signal::kill;

use super::*;

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
