//! The version-control reads adapters call (context C8): what git says about a project.

use super::Facade;
use crate::git::{GitError, GitStatus};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::vcs::ProjectFile;

impl Facade {
    /// A project's working-tree status: what is checked out, how it stands against its
    /// upstream, and every path that differs from the last commit. The snapshot half of
    /// snapshot-then-deltas for version control — pair it with
    /// [`DomainEvent::GitStatusChanged`](crate::events::DomainEvent::GitStatusChanged).
    ///
    /// `Ok(None)` for a project that is not a repository: not keeping a folder under version
    /// control is an ordinary choice, so a surface shows nothing rather than an error. The
    /// remembered status is served when there is one, so several surfaces asking at once cost
    /// one read.
    ///
    /// Reading a repository runs an external tool, so callers reach this through
    /// [`Facade::blocking`] rather than a runtime worker.
    pub fn git_status(&self, project: ProjectId) -> Result<Option<GitStatus>, GitStatusError> {
        let root = self
            .project_root(project)?
            .ok_or(GitStatusError::UnknownProject)?;
        Ok(self.git.status(project, &root)?)
    }

    /// Every path in `project`'s repository: what it tracks, what it does not yet track, and
    /// what it was told to ignore — the answer a surface browsing the project's files renders.
    /// An ignored directory arrives as itself rather than its contents.
    ///
    /// `Ok(None)` for a project that is not a repository, for the same reason
    /// [`Facade::git_status`] gives it. Reading a repository runs an external tool, so callers
    /// reach this through [`Facade::blocking`] rather than a runtime worker.
    pub fn git_files(
        &self,
        project: ProjectId,
    ) -> Result<Option<Vec<ProjectFile>>, GitStatusError> {
        let root = self
            .project_root(project)?
            .ok_or(GitStatusError::UnknownProject)?;
        Ok(self.git.files(project, &root)?)
    }
}

/// Why a project's version-control status could not be read: the project is not open, a durable
/// read failed, or the repository itself could not be read.
#[derive(Debug, thiserror::Error)]
pub enum GitStatusError {
    #[error("no such project")]
    UnknownProject,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Git(#[from] GitError),
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
