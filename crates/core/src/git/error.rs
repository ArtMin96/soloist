//! The git context's typed failures: what a repository operation can refuse to do, stated in
//! terms a surface can act on.
//!
//! Version control's own diagnostics are localized prose, so none of them crosses the port.
//! An adapter classifies what happened into one of these cases and carries only machine data
//! (an exit status) with it — that is the anti-corruption seam that keeps the core's behaviour
//! independent of the wording, and the language, of the tool underneath.

/// Why a git operation did not produce a result.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
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
    /// A failure none of the cases above names: a non-zero exit whose meaning the
    /// machine-readable output did not carry, output that did not parse, or an operating-system
    /// error running the tool. `status` is the exit status where the process produced one.
    #[error("the git command failed")]
    Op { status: Option<i32> },
}
