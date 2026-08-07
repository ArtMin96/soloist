//! Running the system `git` command line under the discipline the core expects of it.
//!
//! Every invocation is machine-readable, bounded, and disposable: it runs in the C locale so
//! nothing it prints is translated, with every way of asking a person for a credential closed off
//! so a missing one fails instead of waiting for an answer nobody can type, in a process group of
//! its own so stopping it stops everything it started, under a time limit and an output ceiling,
//! and it is always reaped — a stopped invocation leaves neither a zombie nor an orphan. The
//! containment itself is [`soloist_exec`], shared with every other adapter that runs an external
//! tool; what is decided here is which invocation to make and what its outcome means.
//!
//! Diagnostics are discarded rather than read. They are prose, and translated; only the exit
//! status crosses back, which is what keeps the core's behaviour independent of the wording,
//! and the language, of a program it does not own. The exceptions are the invocations that run
//! somebody else's code — a commit and the hooks it fires, a switch or a delete version control
//! itself refuses, an exchange with a remote — where what was written names the work in the way or
//! the credential that was missing, and nothing else can. It is carried across as opaque text and
//! never read here, so no behaviour depends on it either; the one place any of it is *classified*
//! rather than carried is [`crate::sync`], which says there why it has no alternative.

use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::time::Duration;

use soloist_core::GitError;
use soloist_exec::{Finished, Run as Bounded, RunError};

/// The command-line tool this adapter drives.
const GIT: &str = "git";

/// How long one invocation may take before it is stopped. Generous enough for a first read of
/// a very large working tree — and for a repository's own commit hooks, which run inside it —
/// bounded so an invocation waiting on something that will never arrive cannot hold its caller
/// for ever.
const TIME_LIMIT: Duration = Duration::from_secs(30);

/// The same for an invocation that reaches another machine, where the time is spent on a network
/// and a server rather than on this disk: a first push of a long history over a slow uplink is
/// legitimately slower than anything local. Still bounded, because a remote that accepts a
/// connection and then says nothing would otherwise hold its caller for ever.
pub(crate) const NETWORK_TIME_LIMIT: Duration = Duration::from_secs(120);

/// The most output one invocation may produce. A working tree past this is past anything a
/// surface could render, and reading a pipe without a ceiling is how a pathological repository
/// becomes an out-of-memory.
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

/// The most of a refused invocation's own account of itself that is carried back. Enough for a
/// hook to say what it objected to, bounded so a hook that prints a whole build log does not
/// become the error message.
const DIAGNOSTIC_LIMIT: usize = 8 * 1024;

/// What one invocation is handed and what is accepted back from it.
pub(crate) struct Run<'a> {
    /// What to write to its standard input, for the invocations that read a patch from there.
    pub input: Option<&'a str>,
    /// An exit status to read as an answer alongside zero, for the invocations whose non-zero
    /// exit reports what they found rather than that they failed.
    pub accepted: Option<i32>,
    /// Whether to carry back what the invocation wrote about itself when it fails — set only
    /// where that text is the user's own, never to be read as a diagnostic.
    pub report_refusal: bool,
    /// How long it may take before it is stopped. Every invocation has one; the invocations that
    /// reach another machine are the only ones that ask for more than the local limit.
    pub time_limit: Duration,
    /// Looked at while the invocation waits, for the ones somebody may change their mind about.
    pub stopped: Option<&'a dyn Fn() -> bool>,
    /// Environment to add on top of what every invocation gets — the invocations that reach a
    /// remote are the only ones that need any, and what they need is not to prompt.
    pub env: &'a [(&'a str, &'a str)],
}

impl Default for Run<'_> {
    fn default() -> Self {
        Self {
            input: None,
            accepted: None,
            report_refusal: false,
            time_limit: TIME_LIMIT,
            stopped: None,
            env: &[],
        }
    }
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
    let mut command = Command::new(GIT);
    command
        .args(args)
        .current_dir(root)
        .env("LC_ALL", "C")
        // There is no terminal behind any invocation Soloist makes, so a prompt on one would be a
        // question asked into nothing. Every other way of asking is the caller's decision, since a
        // person may well be sitting in front of the window that asked (see `crate::sync`).
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(options.env.iter().copied());

    let finished = soloist_exec::run(
        command,
        Bounded {
            input: options.input,
            stopped: options.stopped,
            time_limit: options.time_limit,
            output_limit: OUTPUT_LIMIT,
            diagnostics: options.report_refusal.then_some(DIAGNOSTIC_LIMIT),
        },
    )
    .map_err(failure)?;

    if answered(finished.status, options.accepted) {
        return Ok(finished.output);
    }
    let account = if options.report_refusal {
        account_of(&finished)
    } else {
        String::new()
    };
    if account.is_empty() {
        Err(GitError::Op {
            status: finished.status.code(),
        })
    } else {
        Err(GitError::Refused { output: account })
    }
}

/// What a refused invocation wrote about itself, for the callers that carry that back.
///
/// Both streams are looked at, because which one an invocation uses is its own business: a hook
/// that rejects a commit writes to the diagnostic stream, while `stash pop` reporting a collision
/// writes its account to standard output. The fallback is bounded to the same ceiling, so the
/// invocation whose *output* is what is being read cannot turn its whole answer into an error
/// message.
fn account_of(finished: &Finished) -> String {
    if !finished.diagnostics.is_empty() {
        return finished.diagnostics.clone();
    }
    let written = &finished.output[..finished.output.len().min(DIAGNOSTIC_LIMIT)];
    String::from_utf8_lossy(written).trim().to_string()
}

/// What a run that never reached an end means for a repository read: the tool not being installed
/// at all, running out of time, or a failure whose only machine data is an exit status.
fn failure(err: RunError) -> GitError {
    match err {
        RunError::Spawn(io::ErrorKind::NotFound) => GitError::GitMissing,
        RunError::TimedOut => GitError::Timeout,
        RunError::Stopped => GitError::Stopped,
        RunError::OverLimit { status } => GitError::Op { status },
        RunError::Spawn(_) | RunError::Lost => GitError::Op { status: None },
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

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
