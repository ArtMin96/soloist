//! Drafting text with the user's own agent tool (context C8): what to ask it, and what a refusal
//! means.
//!
//! Both things Soloist can draft — a commit message and a pull request's description — are the same
//! tool run the same way and differ only in what is composed to ask it about, so they share one
//! refusal vocabulary and one place that resolves *which* tool. What each of them composes stays
//! with the surface it belongs to.

use std::path::PathBuf;
use std::sync::Arc;

use super::Facade;
use crate::agents::{AgentTool, OneShotError};
use crate::git::{GitDraftError, PullRequestError};
use crate::ids::ProjectId;
use crate::ports::StoreError;

impl Facade {
    /// Drafts a commit message describing what is staged in `project`, by running the agent tool
    /// the user picked for it.
    ///
    /// Opt-in twice over: it is refused outright until a tool is selected
    /// ([`Facade::set_assist_settings`]), and the project must be trusted, because what runs is an
    /// agent CLI with the project as its working directory. The draft is **only text** — nothing
    /// here stages, commits, or writes anything, and the caller is expected to read and change it
    /// before it is used.
    ///
    /// Composing what to ask reads the repository and the durable settings, so that half goes to
    /// the blocking pool; the run itself is bounded by the agents context and reaches its tool off
    /// the runtime. Must run within a `tokio` runtime.
    pub async fn git_draft_commit_message(
        self: &Arc<Self>,
        project: ProjectId,
    ) -> Result<String, DraftError> {
        let (tool, root, prompt) = self
            .blocking(move |facade| facade.commit_message_question(project))
            .await?;
        Ok(self.agents.draft(&tool, &root, &prompt).await?)
    }

    /// What to ask, and of which tool: the selected tool resolved from the registry, the project's
    /// root, and the prompt composed from what is staged there. Every refusal a draft can produce
    /// without running anything happens here — which is what keeps an unselected tool from costing
    /// a subprocess, let alone an agent.
    fn commit_message_question(
        &self,
        project: ProjectId,
    ) -> Result<(AgentTool, PathBuf, String), DraftError> {
        let selected = self
            .settings
            .get(&())?
            .assist
            .tool
            .ok_or(DraftError::NoAssistTool)?;
        let tool = self
            .agents
            .tool(&selected)?
            .ok_or(DraftError::UnknownTool)?;
        let root = self
            .project_root(project)?
            .ok_or(DraftError::UnknownProject)?;
        let prompt = self.git.commit_message_prompt(project, &root)?;
        Ok((tool, root, prompt))
    }
}

/// Why nothing was drafted: nobody has picked a tool to draft with, the picked tool is no longer
/// in the registry, the project is not open, there was nothing worth describing, or the tool itself
/// could not answer.
///
/// One vocabulary for both drafts — a commit message and a pull request's description — because
/// they are the same tool run the same way, and differ only in what could not be composed to ask
/// it about.
#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    #[error("no agent tool is selected to draft with")]
    NoAssistTool,
    #[error("the agent tool selected to draft with is no longer configured")]
    UnknownTool,
    #[error("no such project")]
    UnknownProject,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Draft(#[from] GitDraftError),
    #[error(transparent)]
    Description(#[from] PullRequestError),
    #[error(transparent)]
    OneShot(#[from] OneShotError),
}

#[cfg(test)]
#[path = "git_draft_tests.rs"]
mod tests;
