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
//! and the language, of a program it does not own. The one exception is an invocation that runs
//! the *user's* code — a commit, and the hooks it fires — where what was written is the user's
//! own message and the only useful thing to show them. It is carried across as opaque text and
//! never read here, so no behaviour depends on it either.

use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use soloist_core::GitError;

/// The command-line tool this adapter drives.
const GIT: &str = "git";

/// How long one invocation may take before it is stopped. Generous enough for a first read of
/// a very large working tree — and for a repository's own commit hooks, which run inside it —
/// bounded so an invocation waiting on something that will never arrive cannot hold its caller
/// for ever.
const TIME_LIMIT: Duration = Duration::from_secs(30);

/// The most output one invocation may produce. A working tree past this is past anything a
/// surface could render, and reading a pipe without a ceiling is how a pathological repository
/// becomes an out-of-memory.
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// The most of a refused invocation's own account of itself that is carried back. Enough for a
/// hook to say what it objected to, bounded so a hook that prints a whole build log does not
/// become the error message.
const DIAGNOSTIC_LIMIT: usize = 8 * 1024;

/// What one invocation is handed and what is accepted back from it.
#[derive(Default)]
pub(crate) struct Run<'a> {
    /// What to write to its standard input, for the invocations that read a patch from there.
    pub input: Option<&'a str>,
    /// An exit status to read as an answer alongside zero, for the invocations whose non-zero
    /// exit reports what they found rather than that they failed.
    pub accepted: Option<i32>,
    /// Whether to carry back what the invocation wrote about itself when it fails — set only
    /// where that text is the user's own, never to be read as a diagnostic.
    pub report_refusal: bool,
}

/// Runs `git args` in `root` and returns what it wrote to standard output.
pub(crate) fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    run_with(root, args, Run::default())
}

/// The same, accepting `accepted` as an answer alongside a zero status.
pub(crate) fn run_accepting(
    root: &Path,
    args: &[&str],
    accepted: Option<i32>,
) -> Result<Vec<u8>, GitError> {
    run_with(
        root,
        args,
        Run {
            accepted,
            ..Run::default()
        },
    )
}

/// Runs `git args` in `root` under `options`.
pub(crate) fn run_with(root: &Path, args: &[&str], options: Run<'_>) -> Result<Vec<u8>, GitError> {
    let mut child = Command::new(GIT)
        .args(args)
        .current_dir(root)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if options.input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(if options.report_refusal {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .process_group(0)
        .spawn()
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => GitError::GitMissing,
            _ => GitError::Op { status: None },
        })?;

    let writing = options.input.map(|input| write_input(&mut child, input));
    let reading = child.stderr.take().map(read_diagnostics);
    let finished = wait_bounded(child, TIME_LIMIT, writing);

    let refusal = reading.and_then(|reading| reading.join().ok());
    let (output, status) = finished?;
    if answered(status, options.accepted) {
        return Ok(output);
    }
    match refusal {
        Some(output) if !output.is_empty() => Err(GitError::Refused { output }),
        _ => Err(GitError::Op {
            status: status.code(),
        }),
    }
}

/// Whether a finished invocation produced an answer: it succeeded, or it exited with the status
/// its caller reads as one.
///
/// An invocation stopped by a signal reports no status at all, and that is never an answer — so
/// it is refused here rather than mistaken for a caller that accepts nothing, which reports no
/// status either.
fn answered(status: ExitStatus, accepted: Option<i32>) -> bool {
    status.success() || (accepted.is_some() && status.code() == accepted)
}

/// Writes `input` to the child's standard input on a thread of its own, closing it afterwards
/// so the child sees the end of what it was given.
///
/// It is a thread and not an inline write because a child that exits before reading all of it
/// would otherwise block the caller for ever; when that happens the write fails and the thread
/// ends, which is why its result is discarded.
fn write_input(child: &mut Child, input: &str) -> JoinHandle<()> {
    let stdin = child.stdin.take();
    let input = input.to_string();
    thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            let _ = stdin.write_all(input.as_bytes());
        }
    })
}

/// Collects what the invocation wrote about itself, bounded, on a thread of its own so it can
/// never fill its pipe and deadlock against the wait.
fn read_diagnostics(mut stderr: ChildStderr) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut written = Vec::new();
        let _ = stderr
            .by_ref()
            .take(DIAGNOSTIC_LIMIT as u64)
            .read_to_end(&mut written);
        String::from_utf8_lossy(&written).trim().to_string()
    })
}

/// Waits for `child` within `limit`, capturing its standard output up to [`OUTPUT_LIMIT`].
///
/// The capture and the wait happen on one thread, so a full pipe can never deadlock against a
/// caller that stopped reading. Whichever way the wait ends — finished, over the output
/// ceiling, or out of time — the process group is signalled and the child is reaped before this
/// returns, and any thread feeding its input is joined, so nothing outlives the call.
fn wait_bounded(
    mut child: Child,
    limit: Duration,
    writing: Option<JoinHandle<()>>,
) -> Result<(Vec<u8>, ExitStatus), GitError> {
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
    if let Some(writing) = writing {
        let _ = writing.join();
    }
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
