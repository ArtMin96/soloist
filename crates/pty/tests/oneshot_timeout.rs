//! What happens to an agent tool that never answers.
//!
//! A binary of its own with a single test, because the assertion is about **this process's**
//! children: `waitpid` over all of them is only unambiguous when nothing else in the binary has
//! spawned one. A drafting run must leave nothing behind — not a process still working on an answer
//! nobody is waiting for, and not a zombie of one.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;
use soloist_core::{AgentOneShot, OneShotError, OneShotInvocation};
use soloist_pty::ShellAgentOneShot;

/// Long enough that the tool is certainly still running when the limit expires.
const FOREVER: &str = "sleep 600";

/// Short enough to keep the test quick, long enough that the shell has started the tool.
const LIMIT: Duration = Duration::from_secs(2);

#[test]
fn a_tool_that_never_answers_is_stopped_and_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("temp dir");

    let refusal = ShellAgentOneShot::with_time_limit(LIMIT)
        .run(
            &OneShotInvocation::in_line(FOREVER.to_string()),
            Path::new(dir.path()),
            &BTreeMap::new(),
        )
        .expect_err("a tool that never answers cannot have answered");

    assert_eq!(
        refusal,
        OneShotError::Timeout,
        "the refusal has to say it ran out of time, or a surface cannot tell the user why",
    );
    // Nothing of the run survives it: a child still running would report itself alive here, and a
    // child killed but never waited on would report itself as a zombie. Neither is an answer of
    // "this process has no children at all".
    assert_eq!(
        waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)),
        Err(Errno::ECHILD),
        "the stopped run was reaped — no orphan, and no zombie",
    );
}
