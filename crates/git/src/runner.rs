//! Running the system `git` command line under the discipline the core expects of it.
//!
//! Every invocation is machine-readable, bounded, and disposable: it runs in the C locale so
//! nothing it prints is translated, with prompting disabled so a missing credential fails
//! instead of waiting for an answer nobody can type, in a process group of its own so stopping
//! it stops everything it started, under a time limit and an output ceiling, and it is always
//! reaped — a stopped invocation leaves neither a zombie nor an orphan. The containment itself is
//! [`soloist_exec`], shared with every other adapter that runs an external tool; what is decided
//! here is which invocation to make and what its outcome means.
//!
//! Diagnostics are discarded rather than read. They are prose, and translated; only the exit
//! status crosses back, which is what keeps the core's behaviour independent of the wording,
//! and the language, of a program it does not own. The one exception is an invocation that runs
//! the *user's* code — a commit, and the hooks it fires — where what was written is the user's
//! own message and the only useful thing to show them. It is carried across as opaque text and
//! never read here, so no behaviour depends on it either.

use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::time::Duration;

use soloist_core::GitError;
use soloist_exec::{Run as Bounded, RunError};

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
    let mut command = Command::new(GIT);
    command
        .args(args)
        .current_dir(root)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0");

    let finished = soloist_exec::run(
        command,
        Bounded {
            input: options.input,
            time_limit: TIME_LIMIT,
            output_limit: OUTPUT_LIMIT,
            diagnostics: options.report_refusal.then_some(DIAGNOSTIC_LIMIT),
        },
    )
    .map_err(failure)?;

    if answered(finished.status, options.accepted) {
        return Ok(finished.output);
    }
    if finished.diagnostics.is_empty() {
        Err(GitError::Op {
            status: finished.status.code(),
        })
    } else {
        Err(GitError::Refused {
            output: finished.diagnostics,
        })
    }
}

/// What a run that never reached an end means for a repository read: the tool not being installed
/// at all, running out of time, or a failure whose only machine data is an exit status.
fn failure(err: RunError) -> GitError {
    match err {
        RunError::Spawn(io::ErrorKind::NotFound) => GitError::GitMissing,
        RunError::TimedOut => GitError::Timeout,
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
