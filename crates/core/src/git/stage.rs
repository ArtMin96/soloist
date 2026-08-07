//! Moving a change across the index, and throwing one away.
//!
//! Every one of these changes the repository, so every one passes the project's trust gate
//! first — reading a working tree runs nothing the project carries, changing one runs its
//! hooks. The gate is spent here, in the context, so the Tauri surface, an agent over MCP, and
//! anything later are refused identically without one of them having to remember to ask.
//!
//! A hunk is named by where it falls rather than by its position in a list, so a request built
//! against a diff the file has since moved past describes a hunk that is no longer there, and
//! is refused rather than applied to whatever occupies those lines now. Constructing the patch
//! that carries a request out belongs to the adapter: the patch format is version control's,
//! and none of it crosses the port.

use std::path::Path;

use crate::ids::ProjectId;
use crate::vcs::{ChangeKind, HunkRange};

use super::error::{GitError, GitWriteError};
use super::path::inside_repository;
use super::repository::GitRepository;
use super::status::Git;

impl Git {
    /// Records everything the working tree holds for `path` in the index, so the next commit
    /// would carry it.
    pub fn stage(&self, project: ProjectId, root: &Path, path: &str) -> Result<(), GitWriteError> {
        self.changing(project, root, path, |repository, original_path| {
            repository.stage(root, path, original_path)
        })
    }

    /// Takes `path` back out of the index, leaving the working tree untouched.
    pub fn unstage(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
    ) -> Result<(), GitWriteError> {
        self.changing(project, root, path, |repository, original_path| {
            repository.unstage(root, path, original_path)
        })
    }

    /// Throws away what the working tree holds for `path` beyond the index, restoring it from
    /// there.
    ///
    /// It reaches no further than the index by construction: a staged change and a commit are
    /// both out of its reach, so the most it can cost is the change the reader was looking at.
    /// A path version control does not track has nothing in the index to be restored from, so
    /// it is refused rather than deleted — Soloist does not delete a file the user made.
    pub fn discard(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
    ) -> Result<(), GitWriteError> {
        self.authorize(project)?;
        if self.is_untracked(project, root, path)? {
            return Err(GitWriteError::UntrackedPath);
        }
        self.changing(project, root, path, |repository, _| {
            repository.discard(root, path)
        })
    }

    /// Records only `hunk` of `path`'s unstaged change in the index, leaving the rest where it
    /// is.
    pub fn stage_hunk(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
        hunk: HunkRange,
    ) -> Result<(), GitWriteError> {
        self.changing(project, root, path, |repository, original_path| {
            repository.stage_hunk(root, path, original_path, hunk)
        })
    }

    /// Takes only `hunk` of `path`'s staged change back out of the index.
    pub fn unstage_hunk(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
        hunk: HunkRange,
    ) -> Result<(), GitWriteError> {
        self.changing(project, root, path, |repository, original_path| {
            repository.unstage_hunk(root, path, original_path, hunk)
        })
    }

    /// Throws away only `hunk` of `path`'s unstaged change, restoring those lines from the
    /// index. Bounded exactly as [`Git::discard`] is: it cannot reach past the index.
    pub fn discard_hunk(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
        hunk: HunkRange,
    ) -> Result<(), GitWriteError> {
        self.changing(project, root, path, |repository, _| {
            repository.discard_hunk(root, path, hunk)
        })
    }

    /// What a change to one path adds to [`Git::mutating`]: the guard that the path names
    /// something inside the repository, and the other name a rename has to be asked about by.
    ///
    /// The path guard runs before the trust gate because it judges only the caller's own input —
    /// it says nothing about the repository, so an untrusted caller learns nothing from it. The
    /// rename is looked up before the gate is taken, because looking it up is itself a read.
    fn changing(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
        act: impl FnOnce(&dyn GitRepository, Option<&str>) -> Result<(), GitError>,
    ) -> Result<(), GitWriteError> {
        if !inside_repository(path) {
            return Err(GitWriteError::OutsideRepository);
        }
        let original_path = self.original_path_of(project, root, path);
        self.mutating(project, |repository| {
            act(repository, original_path.as_deref())
        })
    }

    /// Where a renamed or copied path came from, as the status reports it. Version control
    /// recognises a rename only when both of its names are given: handed one, it sees a file
    /// deleted and an unrelated one appear, and records half the move.
    fn original_path_of(&self, project: ProjectId, root: &Path, path: &str) -> Option<String> {
        self.status(project, root)
            .ok()
            .flatten()?
            .changes
            .iter()
            .find(|change| change.path == path)
            .and_then(|change| change.original_path.clone())
    }

    /// Whether version control does not track `path`, which decides whether there is anything
    /// in the index to restore it from.
    fn is_untracked(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
    ) -> Result<bool, GitWriteError> {
        let Some(status) = self.status(project, root)? else {
            return Ok(false);
        };
        Ok(status
            .changes
            .iter()
            .find(|change| change.path == path)
            .and_then(|change| change.status.unstaged)
            == Some(ChangeKind::Untracked))
    }
}

#[cfg(test)]
#[path = "stage_tests.rs"]
mod tests;
