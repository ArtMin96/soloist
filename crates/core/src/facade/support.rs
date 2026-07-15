//! Session-scoped setup & support actions (context C8): feedback submission and writing
//! the agent guide into the effective project's instructions file.
//!
//! Feedback is global — it is about Soloist, not a project — so it needs no scope. The
//! integration-guide write lands in a project's working tree, so it resolves the session's
//! effective project here, in the core, exactly like every other scoped surface.

use std::path::PathBuf;

use super::scoped::ScopedFacade;
use super::Facade;
use crate::facade::CoordinationError;
use crate::ports::StoreError;
use crate::support::{
    write_integration_guide, FeedbackEntry, FeedbackError, IntegrationFile, IntegrationWrite,
    IntegrationWriteError,
};

/// Why writing the integration guide failed: no project in scope, the project vanished,
/// a durable read failed, or the file write itself.
#[derive(Debug, thiserror::Error)]
pub enum SetupIntegrationError {
    #[error(transparent)]
    Scope(#[from] CoordinationError),
    #[error("no such project")]
    UnknownProject,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Write(#[from] IntegrationWriteError),
}

impl Facade {
    /// Stores a feedback message locally (trimmed, stamped, bounded). Never transmitted —
    /// the entry stays in the local store for the user to review.
    pub fn submit_feedback(&self, message: &str) -> Result<FeedbackEntry, FeedbackError> {
        self.feedback.submit(message)
    }

    /// Every stored feedback entry, oldest first.
    pub fn feedback_list(&self) -> Result<Vec<FeedbackEntry>, StoreError> {
        self.feedback.list()
    }
}

impl ScopedFacade<'_> {
    /// Writes the agent guide into `file` at the session's effective project root as a
    /// managed section (created, appended, or replaced in place — never duplicated).
    pub fn setup_agent_integration(
        &self,
        file: IntegrationFile,
    ) -> Result<IntegrationWrite, SetupIntegrationError> {
        let project = self.coordination_scope()?;
        let root: PathBuf = self
            .inner
            .projects
            .get(project)?
            .ok_or(SetupIntegrationError::UnknownProject)?
            .root;
        Ok(write_integration_guide(&root, file)?)
    }
}

#[cfg(test)]
#[path = "support_tests.rs"]
mod tests;
