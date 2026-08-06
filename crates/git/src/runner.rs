//! Running the system `git` command line under the discipline the core expects of it.
//!
//! Every invocation is machine-readable, bounded, and disposable: it runs in the C locale so
//! nothing it prints is translated, with prompting disabled so a missing credential fails
//! instead of waiting for an answer nobody can type, in a process group of its own so stopping
//! it stops everything it started, under a time limit and an output ceiling, and it is always
//! reaped — a stopped invocation leaves neither a zombie nor an orphan.
//!
//! Diagnostics are discarded rather than read. They are prose, and translated; only the exit
//! status crosses back, which is what keeps the core's behaviour independent of the wording,
//! and the language, of a program it does not own.

use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use soloist_core::GitError;

/// The command-line tool this adapter drives.
const GIT: &str = "git";

/// How long one invocation may take before it is stopped. Generous enough for a first read of
/// a very large working tree, bounded so an invocation waiting on something that will never
/// arrive cannot hold its caller for ever.
const TIME_LIMIT: Duration = Duration::from_secs(30);

/// The most output one invocation may produce. A working tree past this is past anything a
/// surface could render, and reading a pipe without a ceiling is how a pathological repository
/// becomes an out-of-memory.
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// Runs `git args` in `root` and returns what it wrote to standard output.
pub(crate) fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    let child = Command::new(GIT)
        .args(args)
        .current_dir(root)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => GitError::GitMissing,
            _ => GitError::Op { status: None },
        })?;
    let (output, status) = wait_bounded(child, TIME_LIMIT)?;
    if !status.success() {
        return Err(GitError::Op {
            status: status.code(),
        });
    }
    Ok(output)
}

/// Waits for `child` within `limit`, capturing its standard output up to [`OUTPUT_LIMIT`].
///
/// The capture and the wait happen on one thread, so a full pipe can never deadlock against a
/// caller that stopped reading. Whichever way the wait ends — finished, over the output
/// ceiling, or out of time — the process group is signalled and the child is reaped before this
/// returns.
fn wait_bounded(mut child: Child, limit: Duration) -> Result<(Vec<u8>, ExitStatus), GitError> {
    // Spawned into a group of its own, so the child's pid is that group's id.
    let group = Pid::from_raw(child.id() as i32);
    let (finished_tx, finished_rx) = mpsc::channel();
    let capture = thread::spawn(move || {
        let mut output = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            // One byte past the ceiling is all it takes to know the ceiling was crossed.
            let read = stdout
                .by_ref()
                .take(OUTPUT_LIMIT as u64 + 1)
                .read_to_end(&mut output);
            if read.is_err() || output.len() > OUTPUT_LIMIT {
                // Nothing more will be read, so stop the invocation now rather than let it
                // block on a pipe until the time limit expires.
                let _ = killpg(group, Signal::SIGKILL);
            }
        }
        let status = child.wait();
        let _ = finished_tx.send(status.map(|status| (output, status)));
    });
    let finished = finished_rx.recv_timeout(limit);
    if matches!(finished, Err(RecvTimeoutError::Timeout)) {
        let _ = killpg(group, Signal::SIGKILL);
    }
    // Joining after the signal is what makes the reap part of this call: the capture thread
    // only ends once it has waited on the child.
    let _ = capture.join();
    match finished {
        Ok(Ok((output, status))) if output.len() > OUTPUT_LIMIT => Err(GitError::Op {
            status: status.code(),
        }),
        Ok(Ok(finished)) => Ok(finished),
        Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => Err(GitError::Op { status: None }),
        Err(RecvTimeoutError::Timeout) => Err(GitError::Timeout),
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
