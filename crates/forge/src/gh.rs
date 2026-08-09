//! Running the GitHub `gh` command line under the discipline the core expects of it.
//!
//! Every invocation is machine-readable, bounded, and disposable: with every way of asking a person
//! anything closed off — no prompt, no pager, no update notice — in a process group of its own so
//! stopping it stops everything it started, under a time limit and an output ceiling, and always
//! reaped. The containment itself is [`soloist_exec`], shared with every other adapter that runs an
//! external tool; what is decided here is which invocation to make and what its outcome means.
//!
//! Two things cross back from a failure and no more: the exit status, and — where the tool refused
//! — what it wrote about why. The status is machine data. The account is the service's own answer
//! to the user, carried opaque and never read here, so no behaviour depends on the wording of a
//! service Soloist does not own.

use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use soloist_core::ForgeError;
use soloist_exec::{Run as Bounded, RunError, Watch};

/// The command-line tool this adapter drives.
const GH: &str = "gh";

/// How long one invocation may take before it is stopped. Every one of them reaches a service over
/// a network, so the bound is the one a network deserves — generous enough for a slow uplink,
/// bounded because a service that accepts a connection and then says nothing would otherwise hold
/// its caller for ever.
const TIME_LIMIT: Duration = Duration::from_secs(120);

/// The most output one invocation may produce. A pull-request payload past this is past anything a
/// surface could render, and reading a pipe without a ceiling is how one pathological answer
/// becomes an out-of-memory.
const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;

/// The most of a refused invocation's own account of itself that is carried back. Enough for the
/// service to say what it objected to, bounded so a long answer does not become the error message.
const DIAGNOSTIC_LIMIT: usize = 8 * 1024;

/// The exit status the tool uses for "this needs an account and there is none" — its own
/// documented convention, which is why it is the one status read as a meaning rather than carried.
const NEEDS_ACCOUNT: i32 = 4;

/// What one invocation is handed.
#[derive(Default)]
pub(crate) struct Run<'a> {
    /// What to write to its standard input — the description, which is a person's prose and has no
    /// business on a command line.
    pub input: Option<&'a str>,
    /// Looked at while the invocation waits, for the ones somebody may change their mind about.
    pub stopped: Option<&'a dyn Fn() -> bool>,
    /// Who to tell what the invocation is saying about itself while it runs, for the ones somebody
    /// is waiting on the far end of.
    pub watching: Option<Watch<'a>>,
}

/// Runs `gh args` in `root` and returns what it wrote to standard output.
pub(crate) fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>, ForgeError> {
    run_with(root, args, Run::default())
}

/// Runs `gh args` in `root` under `options`.
pub(crate) fn run_with(
    root: &Path,
    args: &[&str],
    options: Run<'_>,
) -> Result<Vec<u8>, ForgeError> {
    let mut command = Command::new(GH);
    command
        .args(args)
        .current_dir(root)
        // Nobody is at a terminal behind any invocation Soloist makes, so every way the tool has of
        // stopping to ask, to page, or to announce something is closed: a question asked into
        // nothing would be waited out to the time limit, and an update notice would arrive in the
        // middle of an answer being read.
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
        .env("NO_COLOR", "1");

    let finished = soloist_exec::run(
        command,
        Bounded {
            input: options.input,
            stopped: options.stopped,
            time_limit: TIME_LIMIT,
            watching: options.watching,
            output_limit: OUTPUT_LIMIT,
            // Always asked for: a refusal's account is the service talking to the user, and it is
            // the only thing that says why a pull request was not opened.
            diagnostics: Some(DIAGNOSTIC_LIMIT),
        },
    )
    .map_err(failure)?;

    if finished.status.success() {
        return Ok(finished.output);
    }
    if finished.status.code() == Some(NEEDS_ACCOUNT) {
        return Err(ForgeError::LoggedOut);
    }
    if finished.diagnostics.is_empty() {
        Err(ForgeError::Op {
            status: finished.status.code(),
        })
    } else {
        Err(ForgeError::Refused {
            output: finished.diagnostics,
        })
    }
}

/// What a run that never reached an end means: the tool not being installed at all, running out of
/// time, being stopped, or a failure whose only machine data is an exit status.
fn failure(err: RunError) -> ForgeError {
    match err {
        RunError::Spawn(io::ErrorKind::NotFound) => ForgeError::Missing,
        RunError::TimedOut => ForgeError::Timeout,
        RunError::Stopped => ForgeError::Stopped,
        RunError::OverLimit { status } => ForgeError::Op { status },
        RunError::Spawn(_) | RunError::Lost => ForgeError::Op { status: None },
    }
}
