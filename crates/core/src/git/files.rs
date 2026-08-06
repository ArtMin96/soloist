//! The listing of everything in a project's repository, for the surface that browses it.
//!
//! Unlike a status, a listing is read only while something is showing it and changes far less
//! often, so it is not remembered — a second cache would be invalidation with no reader. It
//! runs under the project's read gate all the same, so a listing and a status never run
//! against one repository at once.

use std::path::Path;

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::ProjectFile;

use super::error::GitError;
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
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
