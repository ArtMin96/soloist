//! Running an external command under the containment every one of Soloist's adapters owes.
//!
//! A run is disposable: it starts in a process group of its own so stopping it stops everything
//! it started, it finishes within a time limit or is killed, it may produce only so much output,
//! and it is **always reaped** — neither a zombie nor an orphan survives a call. Whichever way a
//! run ends, nothing of it outlives the call that made it.
//!
//! This crate holds only the mechanism. What to run, which environment to run it in, and what a
//! given exit status means are the calling adapter's business: it hands over a configured
//! [`Command`] and reads the outcome back through its own typed error. That split is why the
//! discipline is one implementation rather than one per adapter — nothing here knows what
//! `git`, or an agent CLI, or anything else, is.

use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;

/// What one run is handed, and what is accepted back from it. Every bound is stated by the
/// caller, because what is generous for one command is pathological for another.
pub struct Run<'a> {
    /// What to write to the command's standard input, for a run whose subject arrives that way.
    /// `None` gives it nothing to read, so a command that waits on input fails rather than hangs.
    pub input: Option<&'a str>,
    /// How long the run may take before it is stopped.
    pub time_limit: Duration,
    /// The most standard output the run may produce. Reading a pipe without a ceiling is how one
    /// pathological command becomes an out-of-memory.
    pub output_limit: usize,
    /// How much of what the run wrote *about itself* to carry back, or `None` to discard it.
    /// Diagnostics are prose, and translated, so a caller asks for them only where that text is
    /// worth showing to the person who caused it — never to decide anything.
    pub diagnostics: Option<usize>,
}

/// What a run that reached an end produced. Reaching an end is not succeeding: the status is
/// carried as it was reported, for the caller to read against its own conventions.
#[derive(Debug)]
pub struct Finished {
    pub output: Vec<u8>,
    pub status: ExitStatus,
    /// What the run wrote about itself, bounded, or empty when none was asked for.
    pub diagnostics: String,
}

/// Why a run produced no finished result.
///
/// Machine data only: an error kind, an exit status. Nothing the command printed reaches here, so
/// no caller's behaviour can come to depend on the wording — or the language — of a program
/// Soloist does not own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunError {
    /// The command could not be started at all. `NotFound` is the one worth telling apart: it
    /// means the program is not installed, rather than that running it failed.
    Spawn(io::ErrorKind),
    /// It did not finish within its time limit, and was stopped and reaped.
    TimedOut,
    /// It produced more than its output ceiling allowed, and was stopped and reaped.
    OverLimit { status: Option<i32> },
    /// The wait itself could not report an outcome — the operating system failed the wait, or the
    /// thread carrying it ended without answering.
    Lost,
}

/// Runs `command` under `run`, returning what it produced.
///
/// The caller configures the program, its arguments, its working directory and its environment;
/// the containment is applied here and cannot be opted out of — the standard streams are wired to
/// match `run`, and the child is placed in a process group of its own so a kill reaches whatever
/// it started.
///
/// Blocking, and bounded by `run.time_limit`. Runs the child on threads of its own, so a full
/// pipe can never deadlock against the wait.
pub fn run(mut command: Command, run: Run<'_>) -> Result<Finished, RunError> {
    let mut child = command
        .stdin(if run.input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(if run.diagnostics.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .process_group(0)
        .spawn()
        .map_err(|err| RunError::Spawn(err.kind()))?;

    let writing = run.input.map(|input| write_input(&mut child, input));
    let reading = run
        .diagnostics
        .zip(child.stderr.take())
        .map(|(limit, stderr)| read_diagnostics(stderr, limit));
    let finished = wait_bounded(child, run.time_limit, run.output_limit, writing);

    let diagnostics = reading
        .and_then(|reading| reading.join().ok())
        .unwrap_or_default();
    let (output, status) = finished?;
    Ok(Finished {
        output,
        status,
        diagnostics,
    })
}

/// Writes `input` to the child's standard input on a thread of its own, closing it afterwards so
/// the child sees the end of what it was given.
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

/// Collects what the run wrote about itself, bounded, on a thread of its own so it can never fill
/// its pipe and deadlock against the wait.
fn read_diagnostics(mut stderr: ChildStderr, limit: usize) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut written = Vec::new();
        let _ = stderr.by_ref().take(limit as u64).read_to_end(&mut written);
        String::from_utf8_lossy(&written).trim().to_string()
    })
}

/// Waits for `child` within `limit`, capturing its standard output up to `output_limit`.
///
/// The capture and the wait happen on one thread, so a full pipe can never deadlock against a
/// caller that stopped reading. Whichever way the wait ends — finished, over the output ceiling,
/// or out of time — the process group is signalled and the child is reaped before this returns,
/// and any thread feeding its input is joined, so nothing outlives the call.
fn wait_bounded(
    mut child: Child,
    limit: Duration,
    output_limit: usize,
    writing: Option<JoinHandle<()>>,
) -> Result<(Vec<u8>, ExitStatus), RunError> {
    // Spawned into a group of its own, so the child's pid is that group's id.
    let group = Pid::from_raw(child.id() as i32);
    let (finished_tx, finished_rx) = mpsc::channel();
    let capture = thread::spawn(move || {
        let mut output = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            // One byte past the ceiling is all it takes to know the ceiling was crossed.
            let read = stdout
                .by_ref()
                .take(output_limit as u64 + 1)
                .read_to_end(&mut output);
            if read.is_err() || output.len() > output_limit {
                // Nothing more will be read, so stop the run now rather than let it block on a
                // pipe until the time limit expires.
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
    // Joining after the signal is what makes the reap part of this call: the capture thread only
    // ends once it has waited on the child.
    let _ = capture.join();
    if let Some(writing) = writing {
        let _ = writing.join();
    }
    match finished {
        Ok(Ok((output, status))) if output.len() > output_limit => Err(RunError::OverLimit {
            status: status.code(),
        }),
        Ok(Ok(finished)) => Ok(finished),
        Ok(Err(_)) | Err(RecvTimeoutError::Disconnected) => Err(RunError::Lost),
        Err(RecvTimeoutError::Timeout) => Err(RunError::TimedOut),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
