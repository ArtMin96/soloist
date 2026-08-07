//! The version-control commands and queries adapters call (context C8): what git says about a
//! project, and what a surface may change about it.
//!
//! Reads are ungated. Changes are gated in the core on the user having trusted the project to
//! be changed — a general authorisation kept per project, which version control's write side is
//! the first thing to spend. Every change announces itself the same way the watcher does, by
//! re-reading the status and publishing only if it turned out different, so an action and the
//! watcher noticing the same action converge on one snapshot instead of racing to two.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::Facade;
use crate::agents::{AgentTool, OneShotError};
use crate::events::DomainEvent;
use crate::git::{DiffExtent, Git, GitDraftError, GitError, GitStatus, GitWriteError, Prompting};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::vcs::{Branches, DiffTarget, FileContent, FileDiff, HunkRange, ProjectFile};

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

    /// The branches `project` could switch to, most recently committed to first and bounded at
    /// [`BRANCH_PAGE_SIZE`](crate::git::BRANCH_PAGE_SIZE), plus whether it has anything stashed —
    /// the two things a branch switcher offers.
    ///
    /// `Ok(None)` for a project that is not a repository, for the same reason
    /// [`Facade::git_status`] gives it. A read, so it is ungated; it runs an external tool, so
    /// callers reach it through [`Facade::blocking`].
    pub fn git_branches(&self, project: ProjectId) -> Result<Option<Branches>, GitReadError> {
        let root = self.git_root(project)?;
        Ok(self.git.branches(project, &root)?)
    }

    /// Whether the user has trusted `project` to be changed by Soloist. A surface asks so it can
    /// offer the trust affordance rather than let an action fail; the gate itself is spent in
    /// the core, so a surface that does not ask changes nothing either.
    pub fn is_project_trusted(&self, project: ProjectId) -> Result<bool, GitWriteError> {
        self.git.is_trusted(project)
    }

    /// Records that trust for `project`, which is what the affordance behind
    /// [`Facade::is_project_trusted`] does. One method behind the gate, so every surface grants
    /// it identically.
    ///
    /// It announces itself on the same event a change to the working tree does: what a
    /// version-control surface may show has changed, which is what that event asks a surface to
    /// go and re-read.
    pub fn trust_project(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.trust.trust_project(project)?;
        self.bus.publish(DomainEvent::GitStatusChanged { project });
        Ok(())
    }

    /// Records everything the working tree holds for `path` in the index.
    ///
    /// Runs an external tool, so callers reach this through [`Facade::blocking`] rather than a
    /// runtime worker.
    pub fn git_stage(&self, project: ProjectId, path: &str) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.stage(project, root, path))
    }

    /// Takes `path` back out of the index, leaving the working tree untouched.
    pub fn git_unstage(&self, project: ProjectId, path: &str) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.unstage(project, root, path))
    }

    /// Throws away what the working tree holds for `path` beyond the index. Destructive, and
    /// bounded: it restores from the index, so nothing staged or committed is within its reach.
    /// A path version control does not track is refused rather than deleted.
    pub fn git_discard(&self, project: ProjectId, path: &str) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.discard(project, root, path))
    }

    /// Records only one hunk of `path`'s unstaged change in the index. `hunk` names it by where
    /// it falls, so a request built against a diff the file has moved past is refused.
    pub fn git_stage_hunk(
        &self,
        project: ProjectId,
        path: &str,
        hunk: HunkRange,
    ) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.stage_hunk(project, root, path, hunk)
        })
    }

    /// Takes only one hunk of `path`'s staged change back out of the index.
    pub fn git_unstage_hunk(
        &self,
        project: ProjectId,
        path: &str,
        hunk: HunkRange,
    ) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.unstage_hunk(project, root, path, hunk)
        })
    }

    /// Throws away only one hunk of `path`'s unstaged change. Destructive, and bounded exactly
    /// as [`Facade::git_discard`] is.
    pub fn git_discard_hunk(
        &self,
        project: ProjectId,
        path: &str,
        hunk: HunkRange,
    ) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.discard_hunk(project, root, path, hunk)
        })
    }

    /// Records `project`'s index as a commit carrying `message`, or replaces the last commit
    /// with it when `amend`. The user's hooks, signing and configuration all apply, because it
    /// is their own `git` that runs.
    pub fn git_commit(
        &self,
        project: ProjectId,
        message: &str,
        amend: bool,
    ) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.commit(project, root, message, amend)
        })
    }

    /// Starts a branch called `name` at what `project` has checked out, and switches to it.
    pub fn git_create_branch(&self, project: ProjectId, name: &str) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.create_branch(project, root, name))
    }

    /// Checks out `project`'s branch called `name`. A switch that would overwrite uncommitted work
    /// is refused, carrying version control's own account of what is in the way — nothing is
    /// stashed or discarded to get past it.
    pub fn git_switch_branch(&self, project: ProjectId, name: &str) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.switch_branch(project, root, name))
    }

    /// Removes `project`'s branch called `name`. Destructive, so a surface confirms it first — and
    /// bounded, because a branch holding commits nothing else holds is refused and stays refused.
    pub fn git_delete_branch(&self, project: ProjectId, name: &str) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.delete_branch(project, root, name))
    }

    /// Sets what `project`'s working tree holds aside, leaving it as the last commit left it.
    pub fn git_stash(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.stash(project, root))
    }

    /// Puts `project`'s most recently stashed changes back. A collision with what the working tree
    /// holds now comes back as version control's own account of it, because it left a conflict to
    /// resolve rather than doing what was asked.
    pub fn git_pop_stash(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.pop_stash(project, root))
    }

    /// Hands the checked-out branch's commits to its remote, publishing the branch when it tracks
    /// nothing yet.
    ///
    /// Reaches a machine Soloist has no say over, under the user's own credentials — Soloist keeps
    /// none of its own and names no credential helper. This is the **local user's** door, so
    /// [`Prompting::Allowed`]: they clicked something and are sitting in front of it, so version
    /// control may ask them for a credential. Bounded by the adapter's own limit for reaching a
    /// remote and stoppable before then ([`Facade::git_stop_exchange`]), so callers reach this
    /// through [`Facade::blocking`].
    pub fn git_push(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.push(project, root, Prompting::Allowed)
        })
    }

    /// Brings the remote's commits into `project` and reconciles them with what is checked out,
    /// however the user's own configuration says to. Where they have not said, version control
    /// refuses rather than choosing, and its refusal is what comes back.
    pub fn git_pull(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.pull(project, root, Prompting::Allowed)
        })
    }

    /// Brings the remote's commits in without touching `project`'s working tree, which is what
    /// makes its standing against the upstream true again.
    pub fn git_fetch(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| {
            git.fetch(project, root, Prompting::Allowed)
        })
    }

    /// Asks the exchange with a remote running against `project` to stop.
    ///
    /// Instant and infallible: it sets a signal the exchange looks at, taking nothing that exchange
    /// holds — which is what lets it be called while the exchange is still running, from a surface
    /// whose only other option would be to wait the limit out. The exchange itself then ends as
    /// [`GitError::Stopped`](crate::git::GitError::Stopped), the project's gate is released, and the
    /// next read of the repository goes straight through. Nothing to stop is not a failure.
    pub fn git_stop_exchange(&self, project: ProjectId) {
        self.git.stop_exchange(project);
    }

    /// Abandons a merge that is under way in `project`. Destructive within that merge — a conflict
    /// resolved by hand since it began goes with it — so a surface confirms it first.
    pub fn git_abort_merge(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| git.abort_merge(project, root))
    }

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
    ) -> Result<String, DraftMessageError> {
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
    ) -> Result<(AgentTool, PathBuf, String), DraftMessageError> {
        let selected = self
            .settings
            .get(&())?
            .assist
            .tool
            .ok_or(DraftMessageError::NoAssistTool)?;
        let tool = self
            .agents
            .tool(&selected)?
            .ok_or(DraftMessageError::UnknownTool)?;
        let root = self
            .project_root(project)?
            .ok_or(DraftMessageError::UnknownProject)?;
        let prompt = self.git.commit_message_prompt(project, &root)?;
        Ok((tool, root, prompt))
    }

    /// The shape every version-control change shares: resolve the project's root, make the
    /// change, then re-read the status and announce it if it turned out different.
    fn git_change(
        &self,
        project: ProjectId,
        change: impl FnOnce(&Git, &Path) -> Result<(), GitWriteError>,
    ) -> Result<(), GitWriteError> {
        let root = self
            .project_root(project)?
            .ok_or(GitWriteError::UnknownProject)?;
        change(&self.git, &root)?;
        if matches!(self.git.refresh(project, &root), Ok(true)) {
            self.bus.publish(DomainEvent::GitStatusChanged { project });
        }
        Ok(())
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

/// Why no commit message was drafted: nobody has picked a tool to draft with, the picked tool is
/// no longer in the registry, the project is not open, there was nothing worth describing, or the
/// tool itself could not answer.
#[derive(Debug, thiserror::Error)]
pub enum DraftMessageError {
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
    OneShot(#[from] OneShotError),
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
