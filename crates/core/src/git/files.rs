//! The listing of everything in a project's repository, for the surface that browses it.
//!
//! Unlike a status, a listing is read only while something is showing it and changes far less
//! often, so it is not remembered — a second cache would be invalidation with no reader. It
//! runs under the project's read gate all the same, so a listing and a status never run
//! against one repository at once.

use std::path::Path;

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::{FileContent, ProjectFile};

use super::error::GitError;
use super::path::inside_repository;
use super::status::Git;

impl Git {
    /// Every path in `project`'s repository at `root`: tracked, untracked, and ignored, with an
    /// ignored directory listed as itself rather than walked. `None` for a root that is not a
    /// repository — the same ordinary answer [`Git::status`] gives.
    pub fn files(
        &self,
        project: ProjectId,
        root: &Path,
    ) -> Result<Option<Vec<ProjectFile>>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        match self.repository.list_files(root) {
            Ok(files) => Ok(Some(files)),
            Err(GitError::NotARepo) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// The working tree's copy of one listed path, for a surface that shows a file rather than
    /// a change to one. `None` for a root that is not a repository, for a path that does not
    /// name something inside it, and for one that is no longer there — all of which mean the
    /// same thing to a reader: there is nothing here to show.
    ///
    /// Runs under the project's read gate for the same reason the listing does, and arrives
    /// bounded: a file past the adapter's ceiling is carried only as far as that ceiling, and
    /// says so.
    pub fn file(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
    ) -> Result<Option<FileContent>, GitError> {
        if !inside_repository(path) {
            return Ok(None);
        }
        let gate = self.gate(project);
        let _running = lock(&gate);
        match self.repository.read_file(root, path) {
            Ok(content) => Ok(content),
            Err(GitError::NotARepo) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
