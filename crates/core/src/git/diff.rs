//! What one path's change looks like, and how much of it a reader is handed at once.
//!
//! A diff has no natural size: one commit's worth of a lock file is longer than a surface can
//! render without stalling, and longer than a reader wants to scroll. So a read is capped by
//! default and the reader is told when it was, with the whole of it a second read away. The cap
//! falls on hunk boundaries — a patch that stops halfway through a hunk renders as noise, which
//! is worse than saying there is more.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::{ChangeKind, DiffTarget, FileChange, FileDiff, HunkRange};

use super::error::GitError;
use super::path::inside_repository;
use super::repository::RawFileDiff;
use super::status::Git;

/// The most diff text one capped read carries. Past this a reader is given the first hunks and
/// told there are more, rather than a wall of patch a surface has to lay out before it paints.
const DIFF_LIMIT: usize = 256 * 1024;

/// How much of a diff a read carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffExtent {
    /// Up to [`DIFF_LIMIT`]; a longer diff arrives marked truncated.
    Capped,
    /// All of it, however long — what a reader asks for after being told there was more.
    Full,
}

impl Git {
    /// How `path` differs in `project`'s repository at `root`, `target` deciding against what.
    ///
    /// `None` for a root that is not a repository, and for a path that does not name something
    /// inside it — both are "there is no diff here" rather than faults. An untracked path is
    /// read as the whole of itself whatever `target` asked for, since nothing earlier exists to
    /// compare it against; the answer says which comparison it actually is.
    ///
    /// Runs an external tool, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn diff(
        &self,
        project: ProjectId,
        root: &Path,
        path: &str,
        target: DiffTarget,
        extent: DiffExtent,
    ) -> Result<Option<FileDiff>, GitError> {
        if !inside_repository(path) {
            return Ok(None);
        }
        let Some(status) = self.status(project, root)? else {
            return Ok(None);
        };
        let change = status.changes.iter().find(|change| change.path == path);
        let target = resolve(target, change);
        let original_path = change.and_then(|change| change.original_path.as_deref());

        let gate = self.gate(project);
        let _running = lock(&gate);
        let raw = match self.repository.diff(root, target, path, original_path) {
            Ok(raw) => raw,
            Err(GitError::NotARepo) => return Ok(None),
            Err(err) => return Err(err),
        };
        let carried = assemble(&raw, extent);
        Ok(Some(FileDiff {
            path: path.to_string(),
            original_path: original_path.map(str::to_string),
            target,
            binary: raw.binary,
            patch: carried.patch,
            hunks: carried.hunks,
            truncated: carried.truncated,
        }))
    }
}

/// The comparison a path can actually be read at: the one asked for, unless version control
/// does not track the path, in which case there is only one.
fn resolve(target: DiffTarget, change: Option<&FileChange>) -> DiffTarget {
    match change.and_then(|change| change.status.unstaged) {
        Some(ChangeKind::Untracked) => DiffTarget::Untracked,
        _ => target,
    }
}

/// The part of a diff a reader is given: the patch, where each of its hunks falls, and whether
/// anything was left out.
struct Carried {
    patch: String,
    hunks: Vec<HunkRange>,
    truncated: bool,
}

/// The patch a reader is given, and whether anything was left out of it.
///
/// The first hunk always goes, whatever its size: a header with no hunk under it says a file
/// changed while showing nothing of how, which is the one answer worse than a long one.
///
/// The ranges listed are the hunks actually carried, so an action can only name a hunk the
/// reader was shown.
fn assemble(raw: &RawFileDiff, extent: DiffExtent) -> Carried {
    if raw.binary {
        return Carried {
            patch: String::new(),
            hunks: Vec::new(),
            truncated: false,
        };
    }
    let mut patch = raw.header.clone();
    let mut hunks = Vec::new();
    for (index, hunk) in raw.hunks.iter().enumerate() {
        let over = patch.len() + hunk.text.len() > DIFF_LIMIT;
        if index > 0 && over && matches!(extent, DiffExtent::Capped) {
            return Carried {
                patch,
                hunks,
                truncated: true,
            };
        }
        patch.push_str(&hunk.text);
        hunks.push(hunk.range);
    }
    Carried {
        patch,
        hunks,
        truncated: false,
    }
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
