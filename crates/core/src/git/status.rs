//! The working-tree status surfaces render, and the per-project cache they read it from.
//!
//! Reading a repository costs a subprocess, and several surfaces ask for the same answer, so
//! [`Git`] keeps the last read per project and serves it until something invalidates it. Two
//! bounds hold: exactly one entry per project (dropped with the project), and exactly one read
//! in flight per project — a second caller waits for the first rather than starting a rival
//! subprocess against the same repository.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::{BranchInfo, FileChange};

use super::error::GitError;
use super::repository::GitRepository;

/// A repository's working tree at one moment: what is checked out, and everything that differs
/// from the last commit.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GitStatus {
    /// The checked-out branch and how it stands against its upstream.
    pub branch: BranchInfo,
    /// Every path that differs from the last commit, in the order version control reports them.
    pub changes: Vec<FileChange>,
}

/// The git context (C9): reads a project's repository through the [`GitRepository`] port and
/// remembers the answer.
pub struct Git {
    pub(super) repository: Arc<dyn GitRepository>,
    /// The last read per project. An entry of `None` records a root that is not a repository —
    /// worth remembering, so a project the user does not keep under version control is not
    /// re-read on every glance.
    cached: Mutex<HashMap<ProjectId, Option<GitStatus>>>,
    /// One gate per project, so two callers never run against the same repository at once.
    gates: Mutex<HashMap<ProjectId, Arc<Mutex<()>>>>,
}

impl Git {
    /// Builds the context over the repository port the composition root chose.
    pub fn new(repository: Arc<dyn GitRepository>) -> Self {
        Self {
            repository,
            cached: Mutex::new(HashMap::new()),
            gates: Mutex::new(HashMap::new()),
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
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
