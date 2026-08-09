//! The pull-request commands and queries adapters call (context C8): what can be proposed, what
//! already has been, and the description an agent drafts for it.
//!
//! The one thing that happens here rather than in the git context is resolving the user's own
//! description template: it lives in the coordination library and is selected in this project's
//! settings, neither of which version control knows about. It is resolved into a plain skeleton
//! and handed over as one, so C9 decides which skeleton wins without ever learning what a template
//! is.

use std::path::PathBuf;
use std::sync::Arc;

use super::git_draft::DraftError;
use super::{CoordinationError, Facade};
use crate::agents::AgentTool;
use crate::git::{
    NewPullRequest, Progress, Prompting, PullRequestError, PullRequestSurface, PullRequestTemplate,
};
use crate::ids::ProjectId;
use crate::template::TemplateKind;

impl Facade {
    /// Everything `project`'s pull-request surface needs to decide what to show: whether the forge
    /// can be reached at all, the branch that would be proposed, the branch it would merge into,
    /// the pull request it already has, and the description skeletons on offer.
    ///
    /// A read, so it is ungated. It reaches another machine, so callers come through
    /// [`Facade::blocking`] rather than a runtime worker.
    pub fn git_pull_request_surface(
        &self,
        project: ProjectId,
    ) -> Result<PullRequestSurface, PullRequestError> {
        let root = self.pull_request_root(project)?;
        let fallback = self.pull_request_template(project)?;
        self.git.pull_request_surface(project, &root, fallback)
    }

    /// Proposes what `project` has checked out as a pull request, publishing the branch first when
    /// the remote does not hold it as it stands, and answers with the address of what was made.
    ///
    /// Gated in the core on the user having trusted the project. This is the **local user's** door,
    /// so the push half may stop and ask them for a credential ([`Prompting::Allowed`]) — they
    /// clicked something and are sitting in front of it. Both halves are stoppable through
    /// [`Facade::git_stop_exchange`], and both reach another machine, so callers come through
    /// [`Facade::blocking`].
    pub fn git_create_pull_request(
        &self,
        project: ProjectId,
        new: &NewPullRequest,
    ) -> Result<String, PullRequestError> {
        self.git_propose(project, new, Prompting::Allowed, &Progress::unwatched())
    }

    /// The one route from a façade to a proposal, so which caller may be asked for a credential
    /// while the branch is published is decided where the caller's own authority is known — here
    /// for the local user, and in the session-scoped surface for a caller nobody is watching.
    pub(in crate::facade) fn git_propose(
        &self,
        project: ProjectId,
        new: &NewPullRequest,
        prompting: Prompting,
        progress: &Progress,
    ) -> Result<String, PullRequestError> {
        let root = self.pull_request_root(project)?;
        let created = self
            .git
            .create_pull_request(project, &root, new, prompting, progress)?;
        // Publishing the branch moved how it stands against its upstream, which every
        // version-control surface is watching for.
        self.announce_git(project, &root);
        Ok(created)
    }

    /// Drafts the description for the pull request `project`'s branch would open against `base`,
    /// filling `skeleton` when one was offered, by running the agent tool the user picked.
    ///
    /// Opt-in twice over, exactly as a drafted commit message is: refused until a tool is selected
    /// ([`Facade::set_assist_settings`]) and until the project is trusted. The draft is **only
    /// text** — nothing here proposes anything, and the caller is expected to read and change it
    /// before they do.
    ///
    /// Composing what to ask reads the repository and the durable settings, so that half goes to
    /// the blocking pool; the run itself is bounded by the agents context and reaches its tool off
    /// the runtime. Must run within a `tokio` runtime.
    pub async fn git_draft_pull_request_body(
        self: &Arc<Self>,
        project: ProjectId,
        base: String,
        skeleton: String,
    ) -> Result<String, DraftError> {
        let (tool, root, prompt) = self
            .blocking(move |facade| facade.pull_request_body_question(project, &base, &skeleton))
            .await?;
        Ok(self.agents.draft(&tool, &root, &prompt).await?)
    }

    /// What to ask, and of which tool: the selected tool resolved from the registry, the project's
    /// root, and the prompt composed from what the branch proposes. Every refusal a draft can
    /// produce without running anything happens here.
    fn pull_request_body_question(
        &self,
        project: ProjectId,
        base: &str,
        skeleton: &str,
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
        let prompt = self
            .git
            .pull_request_prompt(project, &root, base, skeleton)?;
        Ok((tool, root, prompt))
    }

    /// The user's own description template for `project`, resolved to the skeleton it seeds — or
    /// `None` when they have selected none, or the selection no longer exists.
    ///
    /// The same resolution every other seedable kind goes through, so a template selected here
    /// behaves like one selected anywhere else and a deleted one quietly stops applying.
    fn pull_request_template(
        &self,
        project: ProjectId,
    ) -> Result<Option<PullRequestTemplate>, PullRequestError> {
        let seed = match self.seed_template(TemplateKind::Pr, project) {
            Ok(seed) => seed,
            // A durable read that failed is a failure; nothing else the resolution can raise is
            // reachable from here, since the kind and the project are both this method's own.
            Err(CoordinationError::Store(err)) => return Err(err.into()),
            Err(_) => None,
        };
        Ok(seed.map(|seed| PullRequestTemplate {
            name: seed.name,
            body: seed.body,
        }))
    }

    /// The folder a project's repository is read from, refusing a project that is not open — one
    /// resolution behind every pull-request call, so all of them refuse identically.
    fn pull_request_root(&self, project: ProjectId) -> Result<PathBuf, PullRequestError> {
        self.project_root(project)?
            .ok_or(PullRequestError::UnknownProject)
    }
}

#[cfg(test)]
#[path = "git_pr_tests.rs"]
mod tests;
