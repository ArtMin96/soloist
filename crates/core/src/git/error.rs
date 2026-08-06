//! The git context's typed failures: what a repository operation can refuse to do, stated in
//! terms a surface can act on.
//!
//! Version control's own diagnostics are localized prose, so none of them crosses the port.
//! An adapter classifies what happened into one of these cases and carries only machine data
//! (an exit status) with it — that is the anti-corruption seam that keeps the core's behaviour
//! independent of the wording, and the language, of the tool underneath.

use crate::ports::StoreError;

/// Why a git operation did not produce a result.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum GitError {
    /// The path is not inside a repository. An ordinary state for a project the user simply
    /// does not keep under version control, not a fault.
    #[error("not a git repository")]
    NotARepo,
    /// No git command-line tool is installed, so nothing can be read at all.
    #[error("the git command-line tool is not installed")]
    GitMissing,
    /// The remote refused the credentials available, or none were available and prompting is
    /// disabled — so the operation failed instead of waiting for an answer nobody can give.
    #[error("git could not authenticate with the remote")]
    AuthFailed,
    /// The operation cannot proceed while the working tree holds an unresolved merge.
    #[error("the working tree has unresolved conflicts")]
    Conflict,
    /// The operation did not finish within its time limit and was stopped.
    #[error("the git command did not finish within its time limit")]
    Timeout,
    /// The hunk an action named is not in the current diff, so nothing was changed. The file
    /// moved on between being read and being acted on — applying the request against the lines
    /// that are there now would change something the caller never saw.
    #[error("that change is no longer there")]
    HunkGone,
    /// Something the operation ran refused it — a hook, most often. `output` is what it wrote:
    /// carried so a surface can show it, never read here, so no behaviour depends on the
    /// wording of a program we do not own.
    #[error("the git command was refused")]
    Refused { output: String },
    /// A failure none of the cases above names: a non-zero exit whose meaning the
    /// machine-readable output did not carry, output that did not parse, or an operating-system
    /// error running the tool. `status` is the exit status where the process produced one.
    #[error("the git command failed")]
    Op { status: Option<i32> },
}

/// Why a change to a project's working tree was not made.
///
/// Reads have [`GitReadError`](crate::facade::GitReadError); a change has more ways to be
/// refused than to fail, and every one of them is decided here in the core rather than by
/// whichever surface asked — so the UI, an agent over MCP, and a future remote caller are
/// refused identically.
#[derive(Debug, thiserror::Error)]
pub enum GitWriteError {
    /// No project by that id is open, so there is no working tree to change. Raised where the
    /// project's root is resolved, which is the façade rather than this context.
    #[error("no such project")]
    UnknownProject,
    /// The user has not authorised Soloist to change this project. Changing a repository runs
    /// the repository's own hooks, so it runs code the project carries — which is the same
    /// thing trusting a command authorises, and is why this is a gate rather than a warning.
    #[error("this project has not been trusted to be changed")]
    Untrusted,
    /// The path named does not name something inside the repository.
    #[error("that path is not inside the repository")]
    OutsideRepository,
    /// The path is not tracked, so there is no earlier version of it to be restored from.
    /// Throwing it away would mean deleting a file nothing else holds a copy of.
    #[error("that file is not tracked, so there is nothing to restore it from")]
    UntrackedPath,
    /// A commit was asked for with nothing but blank space for a message.
    #[error("a commit needs a message")]
    EmptyMessage,
    /// A commit was asked for with nothing staged for it to record.
    #[error("nothing is staged to commit")]
    NothingStaged,
    /// The durable record the authorisation is kept in could not be read.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Version control itself refused, or could not be run.
    #[error(transparent)]
    Git(#[from] GitError),
}
