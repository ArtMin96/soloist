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
//!
//! The one thing it resolves rather than is handed is [`login_shell`], because everything Soloist
//! runs through the user's shell has to agree on which shell that is.

mod shell;

pub use shell::login_shell;

use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;

/// How often a waiting run asks whether it has been asked to stop. Short enough that stopping feels
/// immediate to whoever asked, long enough that waiting costs nothing measurable.
const STOP_CHECK: Duration = Duration::from_millis(25);

/// The most of one remark a watcher is handed. A program that writes without ever ending a line is
/// bounded here rather than allowed to grow a remark until it is the whole output.
const REMARK_LIMIT: usize = 512;

/// The shortest time between two reports about a run, and what every watcher in Soloist uses.
///
/// A watched program describes itself far faster than anything needs to hear it — version control
/// redraws a percentage many times a second — and each report travels to whoever asked for it. Half
/// a second is far below what makes a reader feel a stall and far above what floods anything: a
/// two-minute run is bounded at a few hundred reports rather than the tens of thousands its raw
/// stream would carry. It lives here, beside the mechanism it bounds, so the cadence is one number
/// rather than one per adapter.
pub const REPORT_INTERVAL: Duration = Duration::from_millis(500);

/// What one run is handed, and what is accepted back from it. Every bound is stated by the
/// caller, because what is generous for one command is pathological for another.
pub struct Run<'a> {
    /// What to write to the command's standard input, for a run whose subject arrives that way.
    /// `None` gives it nothing to read, so a command that waits on input fails rather than hangs.
    pub input: Option<&'a str>,
    /// Asked periodically while the run waits, for a run somebody may change their mind about.
    /// Answering true stops it by exactly the path the time limit stops it by — one teardown, two
    /// reasons to reach it — and the outcome says which reason it was.
    pub stopped: Option<&'a dyn Fn() -> bool>,
    /// How long the run may take before it is stopped.
    pub time_limit: Duration,
    /// The most standard output the run may produce. Reading a pipe without a ceiling is how one
    /// pathological command becomes an out-of-memory.
    pub output_limit: usize,
    /// How much of what the run wrote *about itself* to carry back, or `None` to discard it.
    /// Diagnostics are prose, and translated, so a caller asks for them only where that text is
    /// worth showing to the person who caused it — never to decide anything.
    pub diagnostics: Option<usize>,
    /// Who to tell what the run is saying about itself *while* it runs, for a caller with somebody
    /// waiting on the other end of a long one. `None` — the ordinary case — reads and reports
    /// nothing extra, so a run nobody is watching behaves exactly as it did before watching existed.
    pub watching: Option<Watch<'a>>,
}

/// Being told what a run is saying about itself before it ends.
///
/// A run's remarks arrive far faster than anything needs to hear them — version control redraws a
/// percentage many times a second — so what is carried is the **latest** remark rather than every
/// remark, and no more often than `interval`. That is a ceiling on the reporting rather than a
/// promise of it: a run that says nothing is reported on not at all, because a watcher that invents
/// something to say when there is nothing tells its reader less than silence does.
pub struct Watch<'a> {
    /// The shortest time between two reports. Every remark made inside one of these windows is
    /// superseded by the next, so a chatty run costs one report per window rather than hundreds.
    pub interval: Duration,
    /// Handed the latest remark, on the thread that called [`run`]. It is never handed an empty one.
    pub observer: &'a dyn Fn(&str),
}

/// The one remark a watched run has made most recently and nobody has read yet.
///
/// One slot rather than a queue: a superseded remark has no reader and no value, so keeping the
/// latest is both the coalescing and the bound. Written by the thread reading the run's diagnostics,
/// read by the thread waiting on it.
type Latest = Arc<Mutex<Option<String>>>;

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
    /// Whoever asked for it changed their mind, and it was stopped and reaped. Told apart from
    /// running out of time because it is not a failure at all: it is what was asked for.
    Stopped,
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
    // Asked before anything is started as well as while it waits, so a run somebody stopped before
    // it began costs no process at all.
    if run.stopped.is_some_and(|stopped| stopped()) {
        return Err(RunError::Stopped);
    }
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
    let latest = run.watching.as_ref().map(|_| Latest::default());
    let reading = run
        .diagnostics
        .zip(child.stderr.take())
        .map(|(limit, stderr)| match latest.clone() {
            Some(latest) => read_diagnostics_watched(stderr, limit, latest),
            None => read_diagnostics(stderr, limit),
        });
    let finished = wait_bounded(child, &run, writing, latest.as_ref());

    let diagnostics = reading
        .and_then(|reading| reading.join().ok())
        .unwrap_or_default();
    // The last thing a run says is said just before it ends, so without this it would be the one
    // remark nobody ever hears — and a run that ends before the first poll would go unheard
    // entirely. Reading it after the collector has been joined is what makes it the last one.
    if let (Some(watch), Some(latest)) = (&run.watching, &latest) {
        if let Some(remark) = take_latest(latest) {
            (watch.observer)(&remark);
        }
    }
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

/// The same collection, for a run somebody is watching: each remark is published as it completes,
/// while what is *carried back* stays capped at exactly the same `limit` as an unwatched run's.
///
/// Reading continues past that cap rather than stopping at it, because a watcher wants the remark
/// being made now and the cap is on what is kept, not on what is looked at. Everything past the cap
/// is read and dropped, so the memory this holds is bounded by `limit` plus one remark however much
/// the run writes.
fn read_diagnostics_watched(
    mut stderr: ChildStderr,
    limit: usize,
    latest: Latest,
) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut written = Vec::new();
        let mut remark = Vec::new();
        let mut buffer = [0u8; 4096];
        while let Ok(read) = stderr.read(&mut buffer) {
            if read == 0 {
                break;
            }
            let chunk = &buffer[..read];
            let room = limit.saturating_sub(written.len()).min(read);
            written.extend_from_slice(&chunk[..room]);
            for &byte in chunk {
                // Version control ends a progress remark with a carriage return and a finished line
                // with a newline, so both end one.
                if byte == b'\r' || byte == b'\n' {
                    publish(&latest, &mut remark);
                } else if remark.len() < REMARK_LIMIT {
                    remark.push(byte);
                }
            }
        }
        String::from_utf8_lossy(&written).trim().to_string()
    })
}

/// Makes `remark` the latest thing said, and starts the next one. An empty remark is not something
/// said — a run that writes a blank line has nothing to report — so it is dropped.
fn publish(latest: &Latest, remark: &mut Vec<u8>) {
    let said = String::from_utf8_lossy(remark).trim().to_string();
    remark.clear();
    if said.is_empty() {
        return;
    }
    if let Ok(mut slot) = latest.lock() {
        *slot = Some(said);
    }
}

/// Takes the latest remark, leaving nothing behind — so a remark is reported once and silence after
/// it stays silence.
fn take_latest(latest: &Latest) -> Option<String> {
    latest.lock().ok().and_then(|mut slot| slot.take())
}

/// Waits for `child` within the run's time limit, capturing its standard output up to the run's
/// ceiling, and stopping early if the run is asked to stop.
///
/// The capture and the wait happen on one thread, so a full pipe can never deadlock against a
/// caller that stopped reading. Whichever way the wait ends — finished, over the output ceiling,
/// out of time, or asked to stop — the process group is signalled and the child is reaped before
/// this returns, and any thread feeding its input is joined, so nothing outlives the call.
fn wait_bounded(
    mut child: Child,
    run: &Run<'_>,
    writing: Option<JoinHandle<()>>,
    latest: Option<&Latest>,
) -> Result<(Vec<u8>, ExitStatus), RunError> {
    // Spawned into a group of its own, so the child's pid is that group's id.
    let group = Pid::from_raw(child.id() as i32);
    let output_limit = run.output_limit;
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
    let finished = wait_or_stop(&finished_rx, run, latest);
    if matches!(finished, Err(Unfinished::TimedOut | Unfinished::Stopped)) {
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
        Ok(Err(_)) | Err(Unfinished::Lost) => Err(RunError::Lost),
        Err(Unfinished::TimedOut) => Err(RunError::TimedOut),
        Err(Unfinished::Stopped) => Err(RunError::Stopped),
    }
}

/// Why a wait produced nothing to read — before the child's own outcome is judged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Unfinished {
    TimedOut,
    Stopped,
    Lost,
}

/// Waits for the child's outcome, giving up at the time limit or as soon as the run is asked to
/// stop.
///
/// A run nobody can stop and nobody is watching waits in one call, exactly as it always has. One
/// that can be stopped, or is being watched, waits in short steps — because both being asked to stop
/// and having something to report are questions that have to be looked at rather than messages that
/// arrive.
fn wait_or_stop(
    finished: &mpsc::Receiver<io::Result<(Vec<u8>, ExitStatus)>>,
    run: &Run<'_>,
    latest: Option<&Latest>,
) -> Result<io::Result<(Vec<u8>, ExitStatus)>, Unfinished> {
    let watching = run.watching.as_ref().zip(latest);
    if run.stopped.is_none() && watching.is_none() {
        return finished
            .recv_timeout(run.time_limit)
            .map_err(|err| match err {
                RecvTimeoutError::Timeout => Unfinished::TimedOut,
                RecvTimeoutError::Disconnected => Unfinished::Lost,
            });
    }
    let mut left = run.time_limit;
    // Nothing has been reported yet, so the first remark is reported as soon as there is one —
    // waiting out an interval before the first would delay exactly the report that matters most.
    let mut reported: Option<Instant> = None;
    loop {
        if run.stopped.is_some_and(|stopped| stopped()) {
            return Err(Unfinished::Stopped);
        }
        if let Some((watch, latest)) = watching {
            let due = reported.is_none_or(|last| last.elapsed() >= watch.interval);
            if due {
                if let Some(remark) = take_latest(latest) {
                    (watch.observer)(&remark);
                    reported = Some(Instant::now());
                }
            }
        }
        if left.is_zero() {
            return Err(Unfinished::TimedOut);
        }
        let step = left.min(STOP_CHECK);
        match finished.recv_timeout(step) {
            Ok(outcome) => return Ok(outcome),
            Err(RecvTimeoutError::Disconnected) => return Err(Unfinished::Lost),
            Err(RecvTimeoutError::Timeout) => left -= step,
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "watching_tests.rs"]
mod watching_tests;
