//! The version-control reads adapters call (context C8): what git says about a project.

use std::path::PathBuf;

use super::Facade;
use crate::git::{DiffExtent, GitError, GitStatus};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::vcs::{DiffTarget, FileContent, FileDiff, ProjectFile};

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
    pub fn git_status(&self, project: ProjectId) -> Result<Option<GitStatus>, GitReadError> {
        let root = self.git_root(project)?;
        Ok(self.git.status(project, &root)?)
    }

    /// Every path in `project`'s repository: what it tracks, what it does not yet track, and
    /// what it was told to ignore — the answer a surface browsing the project's files renders.
    /// An ignored directory arrives as itself rather than its contents.
    ///
    /// `Ok(None)` for a project that is not a repository, for the same reason
    /// [`Facade::git_status`] gives it. Reading a repository runs an external tool, so callers
    /// reach this through [`Facade::blocking`] rather than a runtime worker.
    pub fn git_files(&self, project: ProjectId) -> Result<Option<Vec<ProjectFile>>, GitReadError> {
        let root = self.git_root(project)?;
        Ok(self.git.files(project, &root)?)
    }

    /// How one path differs, `target` deciding against what and `extent` how much of the answer
    /// is carried. A diff longer than one capped read says so, and asking again at
    /// [`DiffExtent::Full`] carries the rest.
    ///
    /// `Ok(None)` for a project that is not a repository and for a path that does not name
    /// something inside it. Reading a repository runs an external tool, so callers reach this
    /// through [`Facade::blocking`] rather than a runtime worker.
    pub fn git_diff(
        &self,
        project: ProjectId,
        path: &str,
        target: DiffTarget,
        extent: DiffExtent,
    ) -> Result<Option<FileDiff>, GitReadError> {
        let root = self.git_root(project)?;
        Ok(self.git.diff(project, &root, path, target, extent)?)
    }

    /// The working tree's copy of one path, for a surface showing a file rather than a change
    /// to one. Bounded: a file past the adapter's ceiling arrives only as far as that ceiling
    /// and says so, and one holding bytes that are not text arrives with no text at all.
    ///
    /// `Ok(None)` for a project that is not a repository, for a path outside it, and for one
    /// that is no longer there. Reading a file goes through the repository adapter, so callers
    /// reach this through [`Facade::blocking`] rather than a runtime worker.
    pub fn git_file(
        &self,
        project: ProjectId,
        path: &str,
    ) -> Result<Option<FileContent>, GitReadError> {
        let root = self.git_root(project)?;
        Ok(self.git.file(project, &root, path)?)
    }

    /// The folder a project's repository is read from — one resolution behind every read above,
    /// so a project that is not open is refused identically whichever one asked.
    fn git_root(&self, project: ProjectId) -> Result<PathBuf, GitReadError> {
        self.project_root(project)?
            .ok_or(GitReadError::UnknownProject)
    }
}

/// Why a version-control read produced no answer: the project is not open, a durable read
/// failed, or the repository itself could not be read.
#[derive(Debug, thiserror::Error)]
pub enum GitReadError {
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
