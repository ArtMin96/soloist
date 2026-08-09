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

/// The most of a configured commit template that is offered as a starting message.
///
/// Generous for the handful of lines a template actually is, and bounded because what it fills is
/// a box somebody edits by hand: past this it is a document rather than a message, and it is
/// dropped whole rather than cut, since half a template is filled in as though it were all of one.
pub const COMMIT_TEMPLATE_LIMIT: usize = 8 * 1024;

impl Git {
    /// The message a new commit in `project` starts from, as the repository's own configuration
    /// supplies it (`commit.template`). `None` where it supplies none, which is the ordinary case.
    ///
    /// Gated on trust, though it reads rather than changes: the configuration consulted is one the
    /// repository itself can carry, and it names a file anywhere on this disk for Soloist to read
    /// and show. That is the project deciding what Soloist reads, which is the thing trusting a
    /// project authorises — so an untrusted project is refused here rather than quietly answering
    /// with nothing.
    ///
    /// Runs an external tool, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn commit_template(
        &self,
        project: ProjectId,
        root: &Path,
    ) -> Result<Option<String>, GitWriteError> {
        self.authorize(project)?;
        let gate = self.gate(project);
        let _running = lock(&gate);
        Ok(self
            .repository
            .commit_template(root, COMMIT_TEMPLATE_LIMIT)?)
    }

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
