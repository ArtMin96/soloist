//! What a session-scoped caller may do about the project's pull requests (context C8 → C9) — the
//! agent-facing half of the forge surface.
//!
//! Split from the working-tree half for the same reason the local user's surface is: a repository
//! is on this disk and is always there, while a forge is somebody else's service reached through a
//! tool the user may not have installed. The two answer different questions and fail in different
//! ways, so they read as two files rather than one long one.
//!
//! Scope, trust and the credential decision are all the working-tree half's: this surface takes no
//! project either, a merge and a proposal pass the same gate a commit does, and the publish behind
//! a proposal is [`UNATTENDED`](super::scoped_git::UNATTENDED) like every other exchange an agent
//! starts.

use super::scoped::ScopedFacade;
use super::scoped_git::{ScopedGitError, UNATTENDED};
use crate::git::{MergeMethod, NewPullRequest, Progress, PullRequestReview, PullRequestSurface};

impl ScopedFacade<'_> {
    /// What the project's pull-request surface can offer: whether the forge can be reached at all,
    /// the branch that would be proposed, the branch it would merge into, the pull request it
    /// already has, the description skeletons the repository expects, and the ways it allows a
    /// merge.
    ///
    /// A read, so it is ungated. One answer rather than five, so a caller deciding whether to
    /// propose anything spends one call finding out.
    pub fn git_pull_request(&self) -> Result<PullRequestSurface, ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_pull_request_surface(project)?)
    }

    /// What the checked-out branch has open on the service: the pull request, what the checks say,
    /// and what people have written on it — or `None` when it has nothing open.
    pub fn git_pull_request_review(&self) -> Result<Option<PullRequestReview>, ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_pull_request_review(project)?)
    }

    /// Proposes what is checked out as a pull request, publishing the branch first when the remote
    /// does not hold it as it stands, and answers with the address of what was made.
    ///
    /// Gated on the user having trusted the project. The publish half never asks anybody for a
    /// credential, so a branch nobody has arranged credentials for fails promptly rather than
    /// waiting on a question no one is there to answer.
    pub fn git_create_pull_request(&self, new: &NewPullRequest) -> Result<String, ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self.inner.git_propose(project, new, UNATTENDED)?)
    }

    /// Puts pull request `number`'s commits into its base branch by `method`.
    ///
    /// Gated on trust, and never pre-empted: whether the repository's own rules allow it — a check
    /// that has not passed, a review that is owed — is the service's answer, and its refusal is
    /// what comes back.
    pub fn git_merge_pull_request(
        &self,
        number: u64,
        method: MergeMethod,
        progress: &Progress,
    ) -> Result<(), ScopedGitError> {
        let project = self.git_scope()?;
        Ok(self
            .inner
            .git_merge_pull_request(project, number, method, progress)?)
    }
}

#[cfg(test)]
#[path = "scoped_git_pr_tests.rs"]
mod tests;
