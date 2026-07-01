//! Running a composed headless summarizer invocation: the OS behind the core's [`SummaryRunner`].
//!
//! Runs the invocation's command line through the user's login shell (`<shell> -lc <line>`, the
//! same resolution the spawner uses so PATH and version managers resolve), optionally piping the
//! prompt on stdin, and returns the captured stdout. Best-effort and bounded: a spawn failure, a
//! non-zero exit, or a hang past the timeout is a [`SummaryError`], and a child that outlives the
//! timeout is killed and reaped so nothing leaks. The child leads its own process group, so the
//! kill reaches the summarizer CLI the shell spawned, not just the shell. The call blocks (it waits
//! on a child), so the core runs it off the async runtime.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use soloist_core::{SummaryError, SummaryInvocation, SummaryRunner};

use crate::shell::login_shell;

/// How long to wait for the summarizer CLI before treating it as failed. A headless one-shot on a
/// small model returns within seconds; the ceiling only guards a pathological hang.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(45);

/// Runs a composed summarizer invocation through the login shell. Stateless; the timeout bounds
/// each run.
pub struct CommandSummaryRunner {
    timeout: Duration,
}

impl CommandSummaryRunner {
    /// A runner with the default summarizer timeout.
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// A runner with an explicit timeout (tests use a short one to exercise the hang path without
    /// waiting the full default).
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for CommandSummaryRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SummaryRunner for CommandSummaryRunner {
    fn run(&self, invocation: &SummaryInvocation) -> Result<String, SummaryError> {
        run_headless(&login_shell(), invocation, self.timeout)
    }
}

/// Runs `<shell> -lc <command_line>` in its own process group, optionally feeding `stdin`, and
/// returns its captured stdout. Waits for output on a thread so the timeout can kill and reap the
/// whole group if the summarizer hangs; `wait_with_output` drains stdout, so a chatty summarizer
/// cannot wedge the pipe.
fn run_headless(
    shell: &str,
    invocation: &SummaryInvocation,
    timeout: Duration,
) -> Result<String, SummaryError> {
    let mut child = Command::new(shell)
        .arg("-lc")
        .arg(&invocation.command_line)
        .stdin(if invocation.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Lead a new process group (pgid == pid), so the timeout kill below reaches the summarizer
        // CLI the shell spawns, not just the shell.
        .process_group(0)
        .spawn()
        .map_err(|err| SummaryError::Failed(format!("spawn {shell}: {err}")))?;

    let pgid = Pid::from_raw(child.id() as i32);

    if let Some(input) = &invocation.stdin {
        // Feed the prompt and close stdin (dropping the handle) so the summarizer sees EOF; a
        // broken pipe (the CLI exited without reading) is not fatal — the wait reports the outcome.
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
        }
    }

    let (tx, rx) = mpsc::channel();
    let waiter = thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    let result = match rx.recv_timeout(timeout) {
        Ok(Ok(output)) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        Ok(Ok(output)) => Err(SummaryError::Failed(format!(
            "exited with {}",
            output.status
        ))),
        Ok(Err(err)) => Err(SummaryError::Failed(format!("wait: {err}"))),
        Err(RecvTimeoutError::Timeout) => {
            // Past the ceiling: kill and reap the whole group so nothing leaks, then report it.
            let _ = killpg(pgid, Signal::SIGKILL);
            Err(SummaryError::TimedOut)
        }
        Err(RecvTimeoutError::Disconnected) => Err(SummaryError::Failed(
            "summarizer thread ended unexpectedly".into(),
        )),
    };
    // Join so the waiter thread (and, after a kill, the reaped child) never outlives this call.
    let _ = waiter.join();
    result
}

#[cfg(test)]
#[path = "summarize_tests.rs"]
mod tests;
