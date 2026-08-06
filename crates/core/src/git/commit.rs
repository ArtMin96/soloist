//! Recording the index as a commit, and replacing the last one.
//!
//! Almost nothing about a commit is decided here. Who authored it, whether it is signed, which
//! hooks run and what they may refuse are all the user's own configuration, and they apply
//! because the engine underneath is the user's own `git` — the whole reason the adapter drives a
//! command line rather than a library.
//!
//! What is decided here are the two refusals worth making without spending a subprocess: a
//! message that is only blank space, and a first commit with nothing staged for it to record.
//! Both are ordinary mistakes rather than faults, and both are answered the same way for every
//! surface because they are answered in the core.

use std::path::Path;

use crate::ids::ProjectId;
use crate::sync::lock;

use super::error::GitWriteError;
use super::status::Git;

impl Git {
    /// Records `project`'s index as a commit carrying `message`.
    ///
    /// With `amend`, it replaces the last commit instead of adding one — rewriting what is
    /// committed and never touching the working tree, which is why an amend with nothing staged
    /// is ordinary (it is how a message is corrected) while a first commit with nothing staged
    /// is refused.
    ///
    /// Runs an external tool, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker; a
    /// hook of the user's that hangs is stopped by the adapter's time limit rather than held
    /// for ever.
    pub fn commit(
        &self,
        project: ProjectId,
        root: &Path,
        message: &str,
        amend: bool,
    ) -> Result<(), GitWriteError> {
        self.authorize(project)?;
        let message = message.trim();
        if message.is_empty() {
            return Err(GitWriteError::EmptyMessage);
        }
        if !amend && !self.has_staged_change(project, root)? {
            return Err(GitWriteError::NothingStaged);
        }
        let gate = self.gate(project);
        let _running = lock(&gate);
        self.repository.commit(root, message, amend)?;
        Ok(())
    }

    /// Whether anything in `project` is staged for the next commit to record.
    fn has_staged_change(&self, project: ProjectId, root: &Path) -> Result<bool, GitWriteError> {
        let Some(status) = self.status(project, root)? else {
            return Ok(false);
        };
        Ok(status
            .changes
            .iter()
            .any(|change| change.status.staged.is_some()))
    }
}

#[cfg(test)]
#[path = "commit_tests.rs"]
mod tests;
