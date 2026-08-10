//! The working-tree status surfaces render, and the per-project cache they read it from.
//!
//! Reading a repository costs a subprocess, and several surfaces ask for the same answer, so
//! [`Git`] keeps the last read per project and serves it until something invalidates it. Two
//! bounds hold: exactly one entry per project (dropped with the project), and exactly one read
//! in flight per project — a second caller waits for the first rather than starting a rival
//! subprocess against the same repository.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize, Serializer};

use crate::ids::ProjectId;
use crate::ports::{StoreError, TrustRepo};
use crate::sync::lock;
use crate::vcs::{BranchInfo, ChangeKind, FileChange, SyncState};

use super::error::{GitError, GitWriteError};
use super::exchange::Stop;
use super::forge::GitForge;
use super::opener::FileOpener;
use super::repository::GitRepository;

/// A repository's working tree at one moment: what is checked out, and everything that differs
/// from the last commit.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GitStatusFacts {
    /// The checked-out branch and how it stands against its upstream.
    pub branch: BranchInfo,
    /// Every path that differs from the last commit, in the order version control reports them.
    pub changes: Vec<FileChange>,
    /// Whether a merge is under way — which is a separate fact from there being conflicts.
    /// Conflicts also arrive from putting stashed changes back, where there is no merge to
    /// abandon; a merge can be under way with every conflict already resolved. So a surface reads
    /// the conflicts to say what needs attention, and this to say whether abandoning is on offer.
    pub merging: bool,
}

/// Repository facts plus the action projection derived from them when the status crosses a wire.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
pub struct GitStatus {
    #[serde(flatten)]
    facts: GitStatusFacts,
}

/// The actions a surface can offer from one status without asking version control to perform a
/// known no-op or a change it is known to refuse.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct GitCapabilities {
    /// Whether the upstream has commits to bring into the checked-out branch.
    pub pull: bool,
    /// Whether the checked-out branch has commits to hand over, including publishing it first.
    pub push: bool,
    /// Whether the working tree holds tracked work that can be set aside.
    pub stash: bool,
    /// Paths whose unstaged work can be restored from the index.
    #[serde(rename = "discardablePaths")]
    pub discardable_paths: Vec<String>,
}

/// How many changed paths version control reports as created or removed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub struct GitChangeCounts {
    /// Paths created on either side of the index.
    pub added: usize,
    /// Paths removed on either side of the index.
    pub removed: usize,
}

impl GitStatus {
    /// Builds one status from the facts version control reported.
    pub fn new(branch: BranchInfo, changes: Vec<FileChange>, merging: bool) -> Self {
        Self {
            facts: GitStatusFacts {
                branch,
                changes,
                merging,
            },
        }
    }

    /// Takes the repository facts out of the cached status.
    pub fn into_facts(self) -> GitStatusFacts {
        self.facts
    }

    /// Projects action availability from the status facts that prove it. Repository policy,
    /// credentials, hooks, and concurrent changes remain version control's answer when an action
    /// is attempted.
    pub fn capabilities(&self) -> GitCapabilities {
        let named = self.branch.name.is_some();
        let (mut pull, push) = if !named {
            (false, false)
        } else if self.branch.upstream.is_none() {
            (false, true)
        } else {
            match self.branch.sync {
                SyncState::Ahead { .. } => (false, true),
                SyncState::Behind { .. } | SyncState::Diverged { .. } => (true, false),
                SyncState::Unknown | SyncState::UpToDate => (false, false),
            }
        };
        let conflicted = self.changes.iter().any(|change| {
            change.status.staged == Some(ChangeKind::Conflicted)
                || change.status.unstaged == Some(ChangeKind::Conflicted)
        });
        if self.merging || conflicted {
            pull = false;
        }
        let stash = !self.merging
            && !conflicted
            && self.changes.iter().any(|change| {
                change.status.staged.is_some()
                    || change
                        .status
                        .unstaged
                        .is_some_and(|kind| kind != ChangeKind::Untracked)
            });
        let discardable_paths = self
            .changes
            .iter()
            .filter(|change| {
                change.status.unstaged.is_some_and(|kind| {
                    kind != ChangeKind::Untracked && kind != ChangeKind::Conflicted
                })
            })
            .map(|change| change.path.clone())
            .collect();

        GitCapabilities {
            pull,
            push,
            stash,
            discardable_paths,
        }
    }

    /// Counts paths created and removed across the staged and unstaged halves of the status.
    ///
    /// A copied or untracked path is a created path. A rename is the same file moved, so it is
    /// neither created nor removed. An unresolved conflict is neither because the adapter
    /// deliberately collapses git's unmerged modes and therefore cannot state whether the path
    /// exists. One path contributes at most once to each count, even when both halves classify it
    /// the same way; an add followed by a delete contributes to both because the status reports
    /// both changes.
    pub fn change_counts(&self) -> GitChangeCounts {
        let mut counts = GitChangeCounts {
            added: 0,
            removed: 0,
        };
        for change in &self.changes {
            let mut added = false;
            let mut removed = false;
            for kind in [change.status.staged, change.status.unstaged]
                .into_iter()
                .flatten()
            {
                match kind {
                    ChangeKind::Added | ChangeKind::Copied | ChangeKind::Untracked => added = true,
                    ChangeKind::Deleted => removed = true,
                    ChangeKind::Modified
                    | ChangeKind::TypeChanged
                    | ChangeKind::Renamed
                    | ChangeKind::Conflicted => {}
                }
            }
            counts.added += usize::from(added);
            counts.removed += usize::from(removed);
        }
        counts
    }
}

impl Deref for GitStatus {
    type Target = GitStatusFacts;

    fn deref(&self) -> &Self::Target {
        &self.facts
    }
}

impl DerefMut for GitStatus {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.facts
    }
}

impl Serialize for GitStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            #[serde(flatten)]
            facts: &'a GitStatusFacts,
            capabilities: GitCapabilities,
            #[serde(rename = "changeCounts")]
            change_counts: GitChangeCounts,
        }

        Wire {
            facts: &self.facts,
            capabilities: self.capabilities(),
            change_counts: self.change_counts(),
        }
        .serialize(serializer)
    }
}

/// The git context (C9): reads a project's repository through the [`GitRepository`] port and
/// remembers the answer.
pub struct Git {
    pub(super) repository: Arc<dyn GitRepository>,
    /// The hosting service the project's pull requests live on. A second port rather than a
    /// second method on the first, because it is a different machine answering — one the user
    /// may have no tool for and no account on, which is the first thing it is asked.
    pub(super) forge: Arc<dyn GitForge>,
    /// The desktop this machine is, for handing it a file to open. A third port because it is a
    /// third thing answering — neither version control nor the hosting service, but whatever
    /// program the user has registered for that kind of file.
    pub(super) opener: Arc<dyn FileOpener>,
    /// The durable record of which projects the user has authorised Soloist to change. Held
    /// here, in the context that changes them, so no surface can reach a write without passing
    /// the gate — the same reason the supervisor holds it rather than asking a caller.
    trust: Arc<dyn TrustRepo>,
    /// The last read per project. An entry of `None` records a root that is not a repository —
    /// worth remembering, so a project the user does not keep under version control is not
    /// re-read on every glance.
    cached: Mutex<HashMap<ProjectId, Option<GitStatus>>>,
    /// One gate per project, so two callers never run against the same repository at once.
    gates: Mutex<HashMap<ProjectId, Arc<Mutex<()>>>>,
    /// The signal that stops the exchange with a remote currently running against each project.
    /// Held here rather than passed around because whoever changes their mind about an exchange is
    /// never the caller that started it — they arrive later, by another route, with only the project
    /// in hand.
    stops: Mutex<HashMap<ProjectId, Stop>>,
}

impl Git {
    /// Builds the context over the three ports the composition root chose — the working tree on
    /// this disk, the service its pull requests live on, and the desktop a file can be handed to —
    /// and the trust record everything behind the gate is spent against.
    pub fn new(
        repository: Arc<dyn GitRepository>,
        forge: Arc<dyn GitForge>,
        opener: Arc<dyn FileOpener>,
        trust: Arc<dyn TrustRepo>,
    ) -> Self {
        Self {
            repository,
            forge,
            opener,
            trust,
            cached: Mutex::new(HashMap::new()),
            gates: Mutex::new(HashMap::new()),
            stops: Mutex::new(HashMap::new()),
        }
    }

    /// Whether the user has authorised Soloist to make changes within `project`, which every
    /// surface needs to know to say so before an action is refused.
    pub fn is_trusted(&self, project: ProjectId) -> Result<bool, GitWriteError> {
        Ok(self.trusted(project)?)
    }

    /// The one read of the durable authorisation, so nothing in this context asks a second way.
    pub(super) fn trusted(&self, project: ProjectId) -> Result<bool, StoreError> {
        self.trust.is_project_trusted(project)
    }

    /// The check every change to a working tree passes first. Reads are ungated: looking at a
    /// repository runs nothing the repository carries, and changing one runs its hooks.
    pub(super) fn authorize(&self, project: ProjectId) -> Result<(), GitWriteError> {
        if self.trusted(project)? {
            Ok(())
        } else {
            Err(GitWriteError::Untrusted)
        }
    }

    /// `project`'s working-tree status: the remembered one when there is one, otherwise a fresh
    /// read of `root`. `None` for a root that is not a repository — the ordinary answer for a
    /// project without version control, which is why it is not an error.
    pub fn status(&self, project: ProjectId, root: &Path) -> Result<Option<GitStatus>, GitError> {
        if let Some(cached) = lock(&self.cached).get(&project) {
            return Ok(cached.clone());
        }
        self.read(project, root).map(|(status, _)| status)
    }

    /// Re-reads `root` and remembers the result, reporting whether it differs from what was
    /// remembered before. The watcher announces a change only when this says there was one, so
    /// repository churn that leaves the working tree looking the same wakes no surface.
    ///
    /// A failed read leaves the remembered status untouched, so a repository that could not be
    /// read for a moment keeps showing what was true rather than going blank.
    pub fn refresh(&self, project: ProjectId, root: &Path) -> Result<bool, GitError> {
        self.read(project, root).map(|(_, changed)| changed)
    }

    /// Drops everything remembered about `project`, so the cache holds only projects that are
    /// still open.
    pub fn forget(&self, project: ProjectId) {
        lock(&self.cached).remove(&project);
        lock(&self.gates).remove(&project);
        lock(&self.stops).remove(&project);
    }

    /// Asks the exchange with a remote running against `project` to stop, if there is one.
    ///
    /// Takes neither the project's gate nor anything the exchange holds — it sets a flag the
    /// exchange looks at — which is what lets it be called while that exchange is what the gate is
    /// being held for. Nothing to stop is not a failure: an exchange that has already finished is
    /// the outcome the caller wanted.
    pub fn stop_exchange(&self, project: ProjectId) {
        if let Some(stop) = lock(&self.stops).get(&project) {
            stop.stop();
        }
    }

    /// The signal for the exchange about to run against `project`, replacing whichever the last one
    /// left behind. Called with the project's gate held, so the signal on file always belongs to the
    /// exchange that is actually running — and replacing it is what makes a change of mind arriving
    /// after one finished reach nothing: the next exchange never sees it.
    pub(super) fn arm(&self, project: ProjectId) -> Stop {
        let stop = Stop::default();
        lock(&self.stops).insert(project, stop.clone());
        stop
    }

    /// Runs one read under the project's gate and files it, returning the status and whether it
    /// differs from the previous one.
    fn read(&self, project: ProjectId, root: &Path) -> Result<(Option<GitStatus>, bool), GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        let read = match self.repository.status(root) {
            Ok(status) => Some(status),
            Err(GitError::NotARepo) => None,
            Err(err) => return Err(err),
        };
        let mut cached = lock(&self.cached);
        let changed = cached.get(&project) != Some(&read);
        cached.insert(project, read.clone());
        Ok((read, changed))
    }

    /// The project's read gate, created on first use. Held only while a read runs, and always
    /// taken before the cache lock, so the two are never acquired in the opposite order.
    pub(super) fn gate(&self, project: ProjectId) -> Arc<Mutex<()>> {
        lock(&self.gates).entry(project).or_default().clone()
    }

    /// The shape every change to a repository shares: the project's trust gate, then its gate, so
    /// nothing runs beside a read or another change against the same repository.
    ///
    /// Every change goes through here — including the ones that reach the network, which is what
    /// keeps a fetch that will not finish for a minute from being joined by a second one. It is
    /// also the only route to the port's writing side, so a change cannot be made without the
    /// trust gate being spent on it.
    ///
    /// `act` must not read this project's status: the gate is held while it runs, and a read would
    /// wait for a lock its own caller holds.
    pub(super) fn mutating<T>(
        &self,
        project: ProjectId,
        act: impl FnOnce(&dyn GitRepository) -> Result<T, GitError>,
    ) -> Result<T, GitWriteError> {
        self.authorize(project)?;
        let gate = self.gate(project);
        let _running = lock(&gate);
        Ok(act(self.repository.as_ref())?)
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
