//! Proposing a branch's commits as a pull request: what a surface needs before it can offer it,
//! and what happens when it is asked for.
//!
//! Two rules shape everything here.
//!
//! **Nothing is offered that cannot be done.** The forge is a tool the user may not have installed
//! and an account they may not have signed in to, so [`Git::pull_request_surface`] answers that
//! first and spends nothing else when the answer is no. A surface renders what it is told rather
//! than probing for itself.
//!
//! **A repository's own convention wins.** A repository carrying pull-request templates is telling
//! everybody who opens one what it expects; a template the user keeps in Soloist is what they
//! bring to repositories that expect nothing. So the repository's are offered when it has any, and
//! the user's own is the fallback — never both at once, because a choice between "the house style"
//! and "my style" is not a choice anybody wants to make per pull request.
//!
//! The branch proposed is always the one that is checked out, read here rather than accepted from
//! a caller, and it is put on the remote first if it is not there already — by version control,
//! under the user's own credentials, exactly as pressing push would.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::sync::lock;
use crate::vcs::SyncState;

use super::branch::usable_branch_name;
use super::error::{GitError, GitWriteError};
use super::exchange::{Prompting, Stop};
use super::forge::{
    ForgeError, ForgeReadiness, GitForge, NewPullRequest, PullRequest, PullRequestTemplate,
};
use super::review::MergeMethod;
use super::status::Git;

/// Everything a pull-request surface needs to decide what to show, in one read — so it renders
/// once rather than assembling itself from four answers that arrive separately.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PullRequestSurface {
    /// Whether the forge can be reached at all. Anything but [`ForgeReadiness::Ready`] and every
    /// other field is empty: nothing was asked, because nothing could have been answered.
    pub readiness: ForgeReadiness,
    /// The branch whose commits would be proposed, or `None` when nothing is checked out by name —
    /// a detached head has no branch to propose.
    pub head: Option<String>,
    /// The branch it would merge into unless the user says otherwise, or `None` where the forge
    /// does not say and the user has to.
    pub base: Option<String>,
    /// The pull request this branch already has, whatever state it is in, or `None` when it has
    /// none.
    pub existing: Option<PullRequest>,
    /// The description skeletons on offer, in the order to show them: the repository's own when it
    /// carries any, otherwise the user's own if they selected one, otherwise none at all.
    pub templates: Vec<PullRequestTemplate>,
    /// The ways this repository allows a pull request to be put into its base branch, the one it
    /// prefers first — read here rather than at merge time, so the surface offering the merge
    /// already knows what this repository permits.
    pub merge_methods: Vec<MergeMethod>,
}

/// Why a pull request was not proposed, or not prepared for.
///
/// Its own vocabulary rather than the working tree's: the ways this can fail — no forge, no
/// account, nothing checked out to propose — have no counterpart in changing a file, and the
/// surface acts on each of them differently.
#[derive(Debug, thiserror::Error)]
pub enum PullRequestError {
    /// No project by that id is open, so there is no repository to propose from. Raised where the
    /// project's root is resolved, which is the façade rather than this context.
    #[error("no such project")]
    UnknownProject,
    /// The user has not authorised Soloist to act within this project. Proposing a pull request
    /// pushes the branch and runs the repository's own configuration, which is what trust
    /// authorises.
    #[error("this project has not been trusted")]
    Untrusted,
    /// Nothing is checked out by name, so there is no branch to propose.
    #[error("nothing is checked out to propose")]
    DetachedHead,
    /// The branch to merge into is blank, or begins with `-`, which version control would read as
    /// an option rather than a name.
    #[error("that is not a usable branch name")]
    UnusableBranchName,
    /// A pull request was asked for with nothing but blank space for a title.
    #[error("a pull request needs a title")]
    EmptyTitle,
    /// Something was asked about the branch's pull request and it has none open.
    #[error("this branch has no pull request")]
    NoPullRequest,
    /// The check or conversation a handoff named is not on the pull request — it was renamed,
    /// deleted, or belongs to a read the caller has since moved past.
    #[error("that is no longer on this pull request")]
    NoSuchSubject,
    /// The branch holds nothing its base does not, so there is no change to describe.
    #[error("this branch holds nothing its base does not")]
    NothingToDescribe,
    /// The durable record the authorisation is kept in could not be read.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The working tree itself could not be read.
    #[error(transparent)]
    Git(#[from] GitError),
    /// The branch could not be put on the remote, so there was nothing whole to propose. Carried as
    /// the push's own refusal rather than flattened, because what stopped it — a credential, a
    /// remote that refused, a change of mind — is what the user has to act on.
    #[error(transparent)]
    Push(#[from] GitWriteError),
    /// The forge could not be reached, or refused.
    #[error(transparent)]
    Forge(#[from] ForgeError),
}

impl Git {
    /// What `project`'s pull-request surface can offer, `fallback` being the user's own selected
    /// template if they have one.
    ///
    /// A read, so it is ungated. It reaches another machine, so callers come from
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn pull_request_surface(
        &self,
        project: ProjectId,
        root: &Path,
        fallback: Option<PullRequestTemplate>,
    ) -> Result<PullRequestSurface, PullRequestError> {
        let readiness = self.forge.readiness(root);
        if readiness != ForgeReadiness::Ready {
            return Ok(PullRequestSurface {
                readiness,
                head: None,
                base: None,
                existing: None,
                templates: Vec::new(),
                merge_methods: Vec::new(),
            });
        }
        let head = self
            .status(project, root)?
            .and_then(|status| status.branch.name);
        let (repository, templates, existing) = self.asking(project, |forge, _| {
            let repository = forge.repository(root)?;
            let templates = offered(forge.templates(root)?, fallback);
            let existing = match &head {
                Some(head) => forge.pull_request(root, head)?,
                None => None,
            };
            Ok((repository, templates, existing))
        })?;
        Ok(PullRequestSurface {
            readiness,
            head,
            base: repository.default_base,
            existing,
            templates,
            merge_methods: repository.merge_methods,
        })
    }

    /// Proposes what `project` has checked out as a pull request, putting the branch on the remote
    /// first when it is not there as it stands, and answers with the address of what was made.
    ///
    /// Gated on the user having trusted the project: it pushes under their credentials and runs the
    /// repository's own configuration. The push half is the same one pressing push runs, so it is
    /// stoppable and answers to the same `prompting` decision — and so is the proposal itself
    /// ([`Facade::git_stop_exchange`](crate::facade::Facade::git_stop_exchange)), because a service
    /// that accepts a connection and then says nothing would otherwise be waited out to the limit.
    pub fn create_pull_request(
        &self,
        project: ProjectId,
        root: &Path,
        new: &NewPullRequest,
        prompting: Prompting,
    ) -> Result<String, PullRequestError> {
        if !self.trusted(project)? {
            return Err(PullRequestError::Untrusted);
        }
        // Both are the caller's own input rather than anything read from the repository, so they
        // are judged before the trust gate is spent and before a subprocess is.
        if new.title.trim().is_empty() {
            return Err(PullRequestError::EmptyTitle);
        }
        if !usable_branch_name(&new.base) {
            return Err(PullRequestError::UnusableBranchName);
        }
        let status = self.status(project, root)?;
        let head = status
            .as_ref()
            .and_then(|status| status.branch.name.clone())
            .ok_or(PullRequestError::DetachedHead)?;
        // A proposal is about commits the service can see, so anything the remote does not hold
        // goes first — as a publish for a branch it has never seen, which is the same choice
        // pressing push makes and is made from the same remembered status.
        if status.as_ref().is_some_and(unsent) {
            self.push(project, root, prompting)?;
        }
        Ok(self.asking(project, |forge, stop| forge.create(root, &head, new, stop))?)
    }

    /// One request to the forge, under the project's gate so it never runs beside a read or a
    /// change against the same repository, and with the stop signal armed inside that gate — so
    /// whoever changes their mind reaches the request that is actually running.
    pub(super) fn asking<T>(
        &self,
        project: ProjectId,
        act: impl FnOnce(&dyn GitForge, &Stop) -> Result<T, ForgeError>,
    ) -> Result<T, ForgeError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        let stop = self.arm(project);
        act(self.forge.as_ref(), &stop)
    }
}

/// Whether the remote is missing commits the branch holds — a branch it has never seen, or one it
/// has fallen behind. Both are answered from the remembered status rather than by asking the
/// remote again.
fn unsent(status: &super::status::GitStatus) -> bool {
    status.branch.upstream.is_none()
        || matches!(
            status.branch.sync,
            SyncState::Ahead { .. } | SyncState::Diverged { .. }
        )
}

/// Which description skeletons to offer: the repository's own whenever it carries any, and the
/// user's own only where it carries none.
fn offered(
    repository: Vec<PullRequestTemplate>,
    fallback: Option<PullRequestTemplate>,
) -> Vec<PullRequestTemplate> {
    if repository.is_empty() {
        fallback.into_iter().collect()
    } else {
        repository
    }
}

#[cfg(test)]
#[path = "pr_tests.rs"]
mod tests;
