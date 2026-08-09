//! What is checked out, what else could be, and what the working tree can be set aside into.
//!
//! Listing branches is a read, so it is ungated like every other read of a repository. Switching,
//! creating, deleting, and moving work into or out of the stash all change the repository, so each
//! passes the project's trust gate and runs under the project's gate — the same route
//! [`Git::mutating`] gives every change.
//!
//! Nothing here decides what version control would refuse. A switch that would lose work and a
//! delete of a branch holding commits nothing else holds are both refused by the tool, carrying
//! its own account of why, and that account is passed on unread: it names the work in the way,
//! which nothing here could. No change is forced, retried, or set aside to get past one.

use std::path::Path;

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::Branches;

use super::error::{GitError, GitWriteError};
use super::repository::{BranchOp, StashOp};
use super::status::Git;

/// The most branches one read carries. Enough for any repository a person works in by hand, and a
/// ceiling for one carrying a branch per pull request since 2019 — the read is sorted so the ones
/// left out are the ones nobody has committed to in longest.
pub const BRANCH_PAGE_SIZE: usize = 200;

impl Git {
    /// The branches `project` could switch to, and whether it has anything stashed.
    ///
    /// A read, so it is ungated, and bounded at [`BRANCH_PAGE_SIZE`]. `None` for a root that is
    /// not a repository, the same ordinary answer [`Git::status`] gives.
    pub fn branches(&self, project: ProjectId, root: &Path) -> Result<Option<Branches>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        match self.repository.branches(root, BRANCH_PAGE_SIZE) {
            Ok(branches) => Ok(Some(branches)),
            Err(GitError::NotARepo) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Starts a branch called `name` at what is checked out, and switches to it.
    pub fn create_branch(
        &self,
        project: ProjectId,
        root: &Path,
        name: &str,
    ) -> Result<(), GitWriteError> {
        self.branch_op(project, root, BranchOp::Create, name)
    }

    /// Checks out the branch called `name`, leaving what the working tree holds alone.
    ///
    /// A switch that would overwrite work is refused by version control and stays refused: what
    /// the user has not committed is not this app's to stash, discard, or force past.
    pub fn switch_branch(
        &self,
        project: ProjectId,
        root: &Path,
        name: &str,
    ) -> Result<(), GitWriteError> {
        self.branch_op(project, root, BranchOp::Switch, name)
    }

    /// Removes the branch called `name`.
    ///
    /// A branch holding commits no other branch holds is refused, and that refusal is the answer —
    /// there is no forced delete anywhere in this app, so the only way past it is to merge the work
    /// or to run `git` directly.
    pub fn delete_branch(
        &self,
        project: ProjectId,
        root: &Path,
        name: &str,
    ) -> Result<(), GitWriteError> {
        self.branch_op(project, root, BranchOp::Delete, name)
    }

    /// Sets the working tree's changes aside, leaving it as the last commit left it.
    pub fn stash(&self, project: ProjectId, root: &Path) -> Result<(), GitWriteError> {
        self.mutating(project, |repository| repository.stash(root, StashOp::Save))
    }

    /// Puts the most recently stashed changes back into the working tree and forgets them.
    ///
    /// It can collide with what the working tree holds now, and then version control's account of
    /// the collision comes back: the conflict is in the working tree to be resolved and what was
    /// set aside is still there, neither of which a plain success would have said.
    pub fn pop_stash(&self, project: ProjectId, root: &Path) -> Result<(), GitWriteError> {
        self.mutating(project, |repository| repository.stash(root, StashOp::Pop))
    }

    /// The shape the three branch operations share: the name guard, then the gate every change
    /// passes.
    ///
    /// A name that is blank, or that begins with `-` and would therefore be read as an option
    /// rather than a name, is refused here rather than handed on. The guard is in the core because
    /// the caller may not be the local user — an agent's argument is not a place to accept
    /// arbitrary text — and it judges only the caller's own input, so it costs nothing to run
    /// before the trust gate.
    fn branch_op(
        &self,
        project: ProjectId,
        root: &Path,
        op: BranchOp,
        name: &str,
    ) -> Result<(), GitWriteError> {
        if !usable_branch_name(name) {
            return Err(GitWriteError::UnusableBranchName);
        }
        self.mutating(project, |repository| repository.branch(root, op, name))
    }
}

/// Whether `name` can be handed to version control as a branch name at all. Everything else about
/// what makes a name legal is version control's own to judge, and it refuses in its own words.
pub(super) fn usable_branch_name(name: &str) -> bool {
    !name.trim().is_empty() && !name.starts_with('-')
}

#[cfg(test)]
#[path = "branch_tests.rs"]
mod tests;
