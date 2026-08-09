//! What a session-scoped caller may read and change about a project's repository (context C8 →
//! C9) — the agent-facing half of version control.
//!
//! Every method here acts on the session's **effective project** and nothing else. No method takes
//! a project, so there is no project to name: a caller bound to one project cannot address
//! another's repository even by trying, which is the same guarantee the type gives for processes,
//! made cheaper by there being no identifier to guard in the first place.
//!
//! Reads are ungated; changes pass the project's trust gate in C9, exactly as the local user's do.
//! Nothing is confirmed here, because there is nobody to confirm with — a caller reaching this
//! surface is a program, and what stands between it and a discarded change is the trust the user
//! granted the project, not a dialog.
//!
//! The one thing decided differently from the local user's surface is [`UNATTENDED`]: an exchange
//! with a remote must never stop and ask a person for a credential, because no person is waiting
//! at this door.

use std::path::Path;

use super::git::GitReadError;
use super::scoped::ScopedFacade;
use crate::git::{DiffExtent, Git, GitStatus, GitWriteError, Prompting, PullRequestError};
use crate::ids::ProjectId;
use crate::vcs::{Branches, DiffTarget, FileDiff, HunkRange};

/// What an exchange a session-scoped caller starts may do about a credential only a person can
/// give: nothing.
///
/// Named once, and spent by [`ScopedFacade::exchange`] and
/// [`ScopedFacade::git_create_pull_request`] alone, so every remote this surface reaches — a push,
/// a pull, a fetch, the publish behind a proposal — inherits the same answer. A prompt here would
/// be a window opening on a desktop nobody asked to look at, in front of a request nobody is
/// waiting at; a credential already arranged still works, because what is denied is the question
/// rather than the answer.
pub(in crate::facade) const UNATTENDED: Prompting = Prompting::Denied;

/// Why a session-scoped version-control call produced no result.
///
/// The three inner taxonomies are the ones the local user's surface already speaks, kept whole
/// rather than flattened: what a change can be refused for is decided once, in the core, so the
/// UI and an agent are refused identically and only the wire mapping differs.
#[derive(Debug, thiserror::Error)]
pub enum ScopedGitError {
    /// The session has no project in scope, so there is no repository to act on.
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The project in scope is not kept under version control, so there is nothing to read or
    /// change. An ordinary state for a folder, which is why the local user's surface reports it as
    /// an absence — but a caller that asked a question deserves to be told its answer does not
    /// exist rather than handed an empty one.
    #[error("this project is not a git repository")]
    NotARepository,
    /// A read of the working tree failed.
    #[error(transparent)]
    Read(#[from] GitReadError),
    /// A change to the working tree was refused, or failed.
    #[error(transparent)]
    Change(#[from] GitWriteError),
    /// Something about the project's pull requests was refused, or failed.
    #[error(transparent)]
    PullRequest(#[from] PullRequestError),
}

impl ScopedFacade<'_> {
    /// The project's working-tree status: what is checked out, how it stands against its upstream,
    /// every path that differs from the last commit, and whether a merge is under way.
    pub fn git_status(&self) -> Result<GitStatus, ScopedGitError> {
        let project = self.git_scope()?;
        self.inner
            .git_status(project)?
            .ok_or(ScopedGitError::NotARepository)
    }

    /// How one path differs, `target` deciding against what and `extent` how much is carried.
    ///
    /// `None` where the path names nothing inside the repository — which is also the answer for a
    /// project that is not one, since neither has a diff to give.
    pub fn git_diff(
        &self,
        path: &str,
        target: DiffTarget,
        extent: DiffExtent,
    ) -> Result<Option<FileDiff>, ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_diff(project, path, target, extent)?)
    }

    /// The branches that could be switched to, most recently committed to first, and whether
    /// anything is set aside in the stash.
    pub fn git_branches(&self) -> Result<Branches, ScopedGitError> {
        let project = self.git_scope()?;
        self.inner
            .git_branches(project)?
            .ok_or(ScopedGitError::NotARepository)
    }

    /// Records everything the working tree holds for `path` in the index, or only `hunk` of it.
    pub fn git_stage(&self, path: &str, hunk: Option<HunkRange>) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        match hunk {
            Some(hunk) => self.inner.git_stage_hunk(project, path, hunk)?,
            None => self.inner.git_stage(project, path)?,
        }
        Ok(())
    }

    /// Takes `path` — or only `hunk` of it — back out of the index, leaving the working tree alone.
    pub fn git_unstage(&self, path: &str, hunk: Option<HunkRange>) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        match hunk {
            Some(hunk) => self.inner.git_unstage_hunk(project, path, hunk)?,
            None => self.inner.git_unstage(project, path)?,
        }
        Ok(())
    }

    /// Throws away what the working tree holds for `path` — or only `hunk` of it — beyond the
    /// index. Destructive, and bounded: it restores from the index, so nothing staged or committed
    /// is within its reach, and an untracked path is refused rather than deleted.
    pub fn git_discard(&self, path: &str, hunk: Option<HunkRange>) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        match hunk {
            Some(hunk) => self.inner.git_discard_hunk(project, path, hunk)?,
            None => self.inner.git_discard(project, path)?,
        }
        Ok(())
    }

    /// Records the index as a commit carrying `message`, or replaces the last commit with it when
    /// `amend`. The user's own hooks, signing and configuration all apply.
    pub fn git_commit(&self, message: &str, amend: bool) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_commit(project, message, amend)?)
    }

    /// Starts a branch called `name` at what is checked out, and switches to it.
    pub fn git_create_branch(&self, name: &str) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_create_branch(project, name)?)
    }

    /// Checks out the branch called `name`. A switch that would overwrite uncommitted work is
    /// refused, carrying version control's own account — nothing is stashed or discarded to get
    /// past it.
    pub fn git_switch_branch(&self, name: &str) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_switch_branch(project, name)?)
    }

    /// Removes the branch called `name`. Destructive, and bounded: a branch holding commits
    /// nothing else holds is refused and stays refused — there is no forced delete to reach for.
    pub fn git_delete_branch(&self, name: &str) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_delete_branch(project, name)?)
    }

    /// Sets what the working tree holds aside, leaving it as the last commit left it.
    pub fn git_stash(&self) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_stash(project)?)
    }

    /// Puts the most recently stashed changes back. A collision with what the working tree holds
    /// now comes back as version control's own account of it.
    pub fn git_pop_stash(&self) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_pop_stash(project)?)
    }

    /// Hands the checked-out branch's commits to its remote, publishing the branch when it tracks
    /// nothing yet. Never asks anybody for a credential ([`UNATTENDED`]).
    pub fn git_push(&self) -> Result<(), ScopedGitError> {
        self.exchange(Git::push)
    }

    /// Brings the remote's commits in and reconciles them with what is checked out, however the
    /// user's own configuration says to. Never asks anybody for a credential ([`UNATTENDED`]).
    pub fn git_pull(&self) -> Result<(), ScopedGitError> {
        self.exchange(Git::pull)
    }

    /// Brings the remote's commits in without touching the working tree. Never asks anybody for a
    /// credential ([`UNATTENDED`]).
    pub fn git_fetch(&self) -> Result<(), ScopedGitError> {
        self.exchange(Git::fetch)
    }

    /// The one route from this surface to a remote, so [`UNATTENDED`] is spent in a single place
    /// and no exchange an agent starts can be reached any other way.
    fn exchange(
        &self,
        exchange: impl FnOnce(&Git, ProjectId, &Path, Prompting) -> Result<(), GitWriteError>,
    ) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_exchange(project, UNATTENDED, exchange)?)
    }

    /// The project every call here acts on — the session's effective one — or the refusal when it
    /// has none. Resolved here rather than accepted as an argument, which is what makes another
    /// project's repository unreachable from this surface rather than merely guarded.
    pub(in crate::facade) fn git_scope(&self) -> Result<ProjectId, ScopedGitError> {
        self.inner
            .effective_project(self.session)
            .ok_or(ScopedGitError::NoProjectScope)
    }
}

#[cfg(test)]
#[path = "scoped_git_tests.rs"]
mod tests;
