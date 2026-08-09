//! The invocations that reach another machine, and abandoning a merge one of them started.
//!
//! Each is handed straight to version control, so the user's ssh agent, their credential helpers,
//! and whatever `gh auth setup-git` left in their configuration all apply — which is why Soloist
//! keeps no credential of its own and **names no helper**. They run under the network time limit
//! rather than the local one, and they can be stopped part-way, because a remote that accepts a
//! connection and then says nothing would otherwise be waited out to that limit.
//!
//! **Whether a person may be asked for a credential is the caller's to say.** Where somebody is
//! sitting in front of the window that asked, a prompt is the right answer and nothing here gets in
//! its way. Where nobody is — an agent's request, arriving over a socket — every way of asking is
//! closed off, so what would have been a window opening on an unattended desktop is a prompt failure
//! instead. Both answers leave a *non-interactive* helper working exactly as it did: a token already
//! held, an unlocked keyring, `gh auth git-credential`. What is denied is the question, never the
//! answer.
//!
//! **This is the one place in the adapter that reads what version control wrote.** There is no
//! machine-readable signal to read instead: a failure to authenticate and a remote that is simply
//! unreachable share the same exit status, and the difference exists only in the account each gives
//! of itself. So a failed exchange is matched against a few markers — git's own, ssh's, and the one
//! a closed-off prompt produces — and matched only to *classify* it: a marker that is recognised
//! becomes [`GitError::AuthFailed`], and one that is not stays what it was, so an unfamiliar failure
//! is reported as itself rather than as a wrong guess.

use std::path::Path;

use soloist_core::{Exchange, GitError, Prompting, SyncOp};
use soloist_exec::{Watch, REPORT_INTERVAL};

use crate::runner::{self, Run, NETWORK_TIME_LIMIT};

/// What makes version control describe an exchange as it makes it, rather than only when it ends.
///
/// It reports on its own when its error stream is a terminal and stays silent when it is not; every
/// invocation Soloist makes is the second case, so an exchange somebody is waiting on has to ask.
/// Asked for only when somebody is listening, so an exchange nobody is watching runs the same
/// invocation it always did.
const REPORTING: &str = "--progress";

/// The remote a branch that tracks nothing is published to. It is version control's own default —
/// the name it suggests in the hint it prints when a push has no upstream to go to — and any
/// repository arranged differently refuses in its own words rather than being guessed at.
const DEFAULT_REMOTE: &str = "origin";

/// What is published: whatever is checked out, under its own name on the remote.
const CURRENT: &str = "HEAD";

/// What an exchange nobody is watching is run with, so a credential that needs a person fails
/// instead of opening a window on a desktop nobody asked to look at.
///
/// Emptying both askpass variables closes that path whichever of them a configuration named:
/// git-config(1) says of `core.askPass` that it "can be overridden by the GIT_ASKPASS environment
/// variable. If not set, fall back to the value of the SSH_ASKPASS environment variable", so an
/// empty value at the front of that chain leaves nothing to fall back to. ssh's own prompting is
/// closed by its documented never (ssh(1): `SSH_ASKPASS_REQUIRE` set to "never" and "ssh will never
/// attempt to use one").
///
/// The last is a hint to one particular helper rather than a rule: Git Credential Manager documents
/// `GCM_INTERACTIVE` as the environment equivalent of `credential.interactive`, with `0` meaning
/// "never prompt – fail if interaction is required", for "headless and unattended environments …
/// where it would be preferable to fail than to hang indefinitely"
/// (`git-ecosystem/git-credential-manager`, `docs/configuration.md`). Every other helper ignores an
/// environment variable it does not know, so this is additive: nothing here detects a helper,
/// requires one, or prefers one. A *different* interactive helper is not closed by it and is bounded
/// by the time limit alone.
const UNATTENDED: &[(&str, &str)] = &[
    ("GIT_ASKPASS", ""),
    ("SSH_ASKPASS", ""),
    ("SSH_ASKPASS_REQUIRE", "never"),
    ("GCM_INTERACTIVE", "0"),
];

/// What the account of a failed exchange contains when a credential was needed and none could be
/// had, or the one that was had was refused.
///
/// Each is version control's or ssh's own wording, in the C locale every invocation runs in, and
/// each was observed rather than remembered: the disabled prompt (`GIT_TERMINAL_PROMPT=0` reporting
/// `could not read Username for '…': terminal prompts disabled`), an emptied askpass leaving nothing
/// to read, a server that answered a supplied credential with a refusal, and ssh turning down the
/// keys it was offered.
const AUTHENTICATION_MARKERS: &[&str] = &[
    "terminal prompts disabled",
    "could not read Username",
    "could not read Password",
    "unable to read askpass response",
    "Authentication failed",
    "Permission denied (publickey",
];

/// Exchanges commits with the remote as `exchange` asks.
pub(crate) fn sync(root: &Path, exchange: Exchange<'_>) -> Result<(), GitError> {
    let args = invocation(exchange.op, exchange.progress.is_watched());
    let stopped = || exchange.stop.stopped();
    let report = |remark: &str| exchange.progress.report(remark);
    let outcome = runner::run_with(
        root,
        &args,
        Run {
            report_refusal: true,
            time_limit: NETWORK_TIME_LIMIT,
            stopped: Some(&stopped),
            watching: exchange.progress.is_watched().then_some(Watch {
                interval: REPORT_INTERVAL,
                observer: &report,
            }),
            env: match exchange.prompting {
                Prompting::Allowed => &[],
                Prompting::Denied => UNATTENDED,
            },
            ..Run::default()
        },
    );
    match outcome {
        Ok(_) => Ok(()),
        Err(GitError::Refused { output }) if authentication_failed(&output) => {
            Err(GitError::AuthFailed)
        }
        Err(err) => Err(err),
    }
}

/// Abandons a merge that is under way. Its own refusal — there being no merge to abandon — is
/// carried back rather than swallowed, because it means the working tree was not what the caller
/// thought it was.
pub(crate) fn abort_merge(root: &Path) -> Result<(), GitError> {
    runner::run_with(
        root,
        &["merge", "--abort"],
        Run {
            report_refusal: true,
            ..Run::default()
        },
    )
    .map(|_| ())
}

/// What version control is asked to do for `op`, and whether to describe itself while it does it.
///
/// The reporting flag is added only where somebody is listening, so an exchange nobody asked about
/// is invoked with character for character the arguments it was invoked with before any of this
/// existed — and version control, whose error stream is never a terminal here, stays as silent for
/// it as it always was.
fn invocation(op: SyncOp, reporting: bool) -> Vec<&'static str> {
    let mut args = match op {
        SyncOp::Fetch => vec!["fetch"],
        SyncOp::Pull => vec!["pull"],
        SyncOp::Push => vec!["push"],
        // A branch tracking nothing has no upstream to push to, so the remote and the branch are
        // both named, and the branch is recorded as tracking it from then on.
        SyncOp::Publish => vec!["push", "--set-upstream", DEFAULT_REMOTE, CURRENT],
    };
    if reporting {
        args.push(REPORTING);
    }
    args
}

/// Whether a failed exchange's own account of itself says a credential was the problem.
fn authentication_failed(output: &str) -> bool {
    AUTHENTICATION_MARKERS
        .iter()
        .any(|marker| output.contains(marker))
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
