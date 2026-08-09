//! The version-control commands and queries adapters call (context C8): what git says about a
//! project, and what a surface may change about it.
//!
//! Reads are ungated. Changes are gated in the core on the user having trusted the project to
//! be changed — a general authorisation kept per project, which version control's write side is
//! the first thing to spend. Every change announces itself the same way the watcher does, by
//! re-reading the status and publishing only if it turned out different, so an action and the
//! watcher noticing the same action converge on one snapshot instead of racing to two.

use std::path::{Path, PathBuf};

use super::Facade;
use crate::events::DomainEvent;
use crate::git::{DiffExtent, Git, GitError, GitStatus, GitWriteError, Prompting};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::vcs::{Branches, DiffTarget, FileContent, FileDiff, HunkRange, ProjectFile};

/// What every exchange this façade starts is allowed to do about a credential only a person can
/// give: ask them. The local user clicked something and is watching, so a dialog is the right
/// answer for them — the opposite decision, for a caller nobody is watching, is named once in
/// [`scoped_git`](super::scoped_git).
const AT_THE_WINDOW: Prompting = Prompting::Allowed;

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

    /// Hands one of `project`'s files to whatever this machine has registered to open it.
    ///
    /// Gated on trust in the core, because it starts a program the desktop chooses from a name the
    /// repository supplied. A path that leaves the repository — by saying so, or by being a link
    /// that leads out of it — is refused and nothing is opened.
    ///
    /// Runs outside this process, so callers reach this through [`Facade::blocking`] rather than a
    /// runtime worker.
    pub fn git_open_file(&self, project: ProjectId, path: &str) -> Result<(), GitWriteError> {
        let root = self
            .project_root(project)?
            .ok_or(GitWriteError::UnknownProject)?;
        self.git.open_file(project, &root, path)
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

    /// The message a new commit in `project` starts from, as the repository's own configuration
    /// supplies it (`commit.template`), with the guidance lines version control strips from an
    /// edited message already gone. `Ok(None)` where nothing is configured.
    ///
    /// Offered rather than applied: it is the text a message box starts at, and whoever is writing
    /// the commit replaces it. Nothing is committed here, and the commit itself is unchanged —
    /// version control consults a template only when it would open an editor, and Soloist commits
    /// the message it was handed.
    ///
    /// Gated on trust like a change, because the configuration read is one the repository can
    /// carry and it names a file for Soloist to read. Runs an external tool, so callers reach this
    /// through [`Facade::blocking`] rather than a runtime worker.
    pub fn git_commit_template(&self, project: ProjectId) -> Result<Option<String>, GitWriteError> {
        let root = self
            .project_root(project)?
            .ok_or(GitWriteError::UnknownProject)?;
        self.git.commit_template(project, &root)
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
        self.git_exchange(project, AT_THE_WINDOW, Git::push)
    }

    /// Brings the remote's commits into `project` and reconciles them with what is checked out,
    /// however the user's own configuration says to. Where they have not said, version control
    /// refuses rather than choosing, and its refusal is what comes back.
    pub fn git_pull(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_exchange(project, AT_THE_WINDOW, Git::pull)
    }

    /// Brings the remote's commits in without touching `project`'s working tree, which is what
    /// makes its standing against the upstream true again.
    pub fn git_fetch(&self, project: ProjectId) -> Result<(), GitWriteError> {
        self.git_exchange(project, AT_THE_WINDOW, Git::fetch)
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

    /// The one route from a façade to a remote, so which caller may be asked for a credential is
    /// decided in exactly two places — here for the local user, and in the session-scoped surface
    /// for a caller nobody is sitting in front of — rather than at each of the three exchanges.
    ///
    /// `exchange` is one of [`Git::push`], [`Git::pull`] and [`Git::fetch`]; the rest of the
    /// shape (root, gate, announce) is [`Facade::git_change`]'s.
    pub(in crate::facade) fn git_exchange(
        &self,
        project: ProjectId,
        prompting: Prompting,
        exchange: impl FnOnce(&Git, ProjectId, &Path, Prompting) -> Result<(), GitWriteError>,
    ) -> Result<(), GitWriteError> {
        self.git_change(project, |git, root| exchange(git, project, root, prompting))
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
        self.announce_git(project, &root);
        Ok(())
    }

    /// Re-reads `project`'s status and announces it only if it turned out different — the half of
    /// every version-control change that happens after the change itself, so an action and the
    /// watcher noticing that same action converge on one snapshot rather than racing to two.
    pub(super) fn announce_git(&self, project: ProjectId, root: &Path) {
        if matches!(self.git.refresh(project, root), Ok(true)) {
            self.bus.publish(DomainEvent::GitStatusChanged { project });
        }
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
