//! What version control refused, as the wire says it: the closed [`GitRefusal`] vocabulary and
//! the one mapping from the core's version-control taxonomies onto it.
//!
//! Kept apart from the rest of the failure taxonomy ([`crate::error`]) because it is a
//! classification of its own rather than one more variant: a remote refuses a credential, a hook
//! objects, a merge is under way, the user changed their mind — outcomes a caller acts on
//! differently and can only tell apart if they are **named**.
//!
//! Naming them is the point. The Model Context Protocol defines no shape for reporting that an
//! operation was stopped by anybody other than the caller — `notifications/cancelled` travels the
//! other way, and the specification's own error codes name nothing of the sort — so an operation
//! the local user stopped mid-flight would otherwise reach an agent as a sentence to read. A word
//! it can match on is what lets it tell being stopped from having failed without parsing prose.
//!
//! Every one of these words is decided from a closed enum in the core. Nothing here reads what a
//! tool printed: an account a tool or a service wrote is appended to the message for a reader and
//! never looked at.

use serde::{Deserialize, Serialize};
use soloist_core::{
    ForgeError, GitError, GitReadError, GitWriteError, PullRequestError, ScopedGitError,
};

use crate::error::IpcError;

/// Why a version-control call produced no result, as one word a caller can match on.
///
/// A closed set: every case here is something a caller does differently about, and everything
/// else is [`GitRefusal::Failed`] rather than a word invented for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitRefusal {
    /// The project in scope is not kept under version control.
    NotARepository,
    /// No `git` is installed, so nothing can be read or changed at all.
    NotInstalled,
    /// The user has not trusted this project to be changed. Distinct from a command's own trust:
    /// this is the project-wide authorisation version control's write side spends, and only the
    /// user can grant it — asking them is the whole of what a caller can do about it.
    ProjectUntrusted,
    /// The path named does not name something inside the repository.
    OutsideRepository,
    /// The path is not tracked, so there is no earlier version to restore it from.
    UntrackedPath,
    /// The branch name is blank, or begins with `-`, which version control reads as an option.
    UnusableBranchName,
    /// A commit was asked for with nothing but blank space for a message.
    EmptyMessage,
    /// A commit was asked for with nothing staged to record.
    NothingStaged,
    /// The hunk named is no longer in the diff: the file moved on between being read and being
    /// acted on. Re-read the diff and name a hunk from what it reports now.
    StaleHunk,
    /// No credential the remote would accept, and nobody could be asked for one.
    AuthFailed,
    /// The working tree holds an unresolved merge.
    Conflict,
    /// The operation did not finish within its time limit and was stopped.
    TimedOut,
    /// Somebody asked for the operation to stop, and it did. **Not a failure** — it is what was
    /// asked for, by the person at the app rather than by the caller, and it is named so that
    /// nothing reports it as one.
    Stopped,
    /// Version control, one of its hooks, or the service refused. The message carries the account
    /// it wrote, which is the only thing that names what is in the way.
    Refused,
    /// Nothing is checked out by name, so there is no branch to propose.
    DetachedHead,
    /// A pull request was asked for with nothing but blank space for a title.
    EmptyTitle,
    /// The checked-out branch has no pull request open.
    NoPullRequest,
    /// The forge command-line tool is not installed, so pull requests cannot be reached.
    ForgeMissing,
    /// The forge tool is installed but signed in to no account.
    ForgeLoggedOut,
    /// A failure none of the words above names. The message carries whatever was known about it.
    Failed,
}

/// What one core refusal amounts to on the wire.
enum Verdict<'a> {
    /// A version-control refusal, with the account the tool wrote where there was one.
    Refused {
        reason: GitRefusal,
        account: Option<&'a str>,
    },
    /// The caller has no project in scope — the wire's own word, shared with every other scoped
    /// surface, because it is about the session rather than about version control.
    NoProjectScope,
    /// The project named is not open — likewise the wire's own word.
    UnknownProject,
    /// The app could not serve the request; nothing the caller did caused it.
    Internal,
}

impl From<ScopedGitError> for IpcError {
    /// Classifies once and renders once: the reason a caller matches on, and the core's own
    /// sentence for it — extended by the tool's account where one came back, so a hook's objection
    /// reaches whoever has to act on it.
    fn from(err: ScopedGitError) -> Self {
        let message = err.to_string();
        match classify(&err) {
            Verdict::Refused { reason, account } => IpcError::Git {
                reason,
                message: match account {
                    Some(account) => format!("{message}: {account}"),
                    None => message,
                },
            },
            Verdict::NoProjectScope => IpcError::NoProjectScope,
            Verdict::UnknownProject => IpcError::UnknownProject,
            Verdict::Internal => IpcError::Internal(message),
        }
    }
}

fn classify(err: &ScopedGitError) -> Verdict<'_> {
    match err {
        ScopedGitError::NoProjectScope => Verdict::NoProjectScope,
        ScopedGitError::NotARepository => refused(GitRefusal::NotARepository),
        ScopedGitError::Read(err) => read(err),
        ScopedGitError::Change(err) => change(err),
        ScopedGitError::PullRequest(err) => pull_request(err),
    }
}

/// A refusal whose whole account is the core's own sentence.
fn refused<'a>(reason: GitRefusal) -> Verdict<'a> {
    Verdict::Refused {
        reason,
        account: None,
    }
}

/// A refusal carrying what the tool or the service wrote about it.
fn told(reason: GitRefusal, account: &str) -> Verdict<'_> {
    Verdict::Refused {
        reason,
        account: Some(account),
    }
}

fn read(err: &GitReadError) -> Verdict<'_> {
    match err {
        GitReadError::UnknownProject => Verdict::UnknownProject,
        GitReadError::Store(_) => Verdict::Internal,
        GitReadError::Git(err) => git(err),
    }
}

fn change(err: &GitWriteError) -> Verdict<'_> {
    match err {
        GitWriteError::UnknownProject => Verdict::UnknownProject,
        GitWriteError::Store(_) => Verdict::Internal,
        GitWriteError::Untrusted => refused(GitRefusal::ProjectUntrusted),
        GitWriteError::OutsideRepository => refused(GitRefusal::OutsideRepository),
        GitWriteError::UntrackedPath => refused(GitRefusal::UntrackedPath),
        GitWriteError::UnusableBranchName => refused(GitRefusal::UnusableBranchName),
        GitWriteError::EmptyMessage => refused(GitRefusal::EmptyMessage),
        GitWriteError::NothingStaged => refused(GitRefusal::NothingStaged),
        GitWriteError::Git(err) => git(err),
        // Opening a file in the desktop's own application is the local user's door and is not on
        // the session-scoped surface, so this cannot arrive here. Left unnamed rather than given a
        // word of its own, which would be a wire vocabulary nothing can produce.
        GitWriteError::Unopenable => refused(GitRefusal::Failed),
    }
}

fn pull_request(err: &PullRequestError) -> Verdict<'_> {
    match err {
        PullRequestError::UnknownProject => Verdict::UnknownProject,
        PullRequestError::Store(_) => Verdict::Internal,
        PullRequestError::Untrusted => refused(GitRefusal::ProjectUntrusted),
        PullRequestError::DetachedHead => refused(GitRefusal::DetachedHead),
        PullRequestError::UnusableBranchName => refused(GitRefusal::UnusableBranchName),
        PullRequestError::EmptyTitle => refused(GitRefusal::EmptyTitle),
        PullRequestError::NoPullRequest => refused(GitRefusal::NoPullRequest),
        PullRequestError::Git(err) => git(err),
        PullRequestError::Push(err) => change(err),
        PullRequestError::Forge(err) => forge(err),
        // Handing a check or a conversation to an agent, and drafting a description, are both the
        // local user's doors and are not on the session-scoped surface either.
        PullRequestError::NoSuchSubject | PullRequestError::NothingToDescribe => {
            refused(GitRefusal::Failed)
        }
    }
}

fn git(err: &GitError) -> Verdict<'_> {
    match err {
        GitError::NotARepo => refused(GitRefusal::NotARepository),
        GitError::GitMissing => refused(GitRefusal::NotInstalled),
        GitError::AuthFailed => refused(GitRefusal::AuthFailed),
        GitError::Conflict => refused(GitRefusal::Conflict),
        GitError::Timeout => refused(GitRefusal::TimedOut),
        GitError::Stopped => refused(GitRefusal::Stopped),
        GitError::HunkGone => refused(GitRefusal::StaleHunk),
        GitError::Refused { output } => told(GitRefusal::Refused, output),
        GitError::Op { .. } => refused(GitRefusal::Failed),
    }
}

fn forge(err: &ForgeError) -> Verdict<'_> {
    match err {
        ForgeError::Missing => refused(GitRefusal::ForgeMissing),
        ForgeError::LoggedOut => refused(GitRefusal::ForgeLoggedOut),
        ForgeError::Refused { output } => told(GitRefusal::Refused, output),
        ForgeError::Timeout => refused(GitRefusal::TimedOut),
        ForgeError::Stopped => refused(GitRefusal::Stopped),
        ForgeError::Op { .. } => refused(GitRefusal::Failed),
    }
}

#[cfg(test)]
#[path = "vcs_error_tests.rs"]
mod tests;
