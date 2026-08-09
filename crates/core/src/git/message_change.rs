//! How a staged change is described to an agent: by its own patches while they fit, and by its
//! paths once they do not.
//!
//! Not everything staged says anything. A resolver's own record of what it picked, and a file a
//! tool wrote rather than a person, describe no intent — they are noise that costs prompt and
//! misleads a reader of it, so they are left out before anything is read.
//!
//! The fallback falls on the whole change rather than on the tail of it: a summary of everything
//! that moved is a better description than a complete diff of the first tenth.

use std::path::Path;

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::{ChangeKind, DiffTarget, FileChange};

use super::error::GitError;
use super::status::Git;

/// File names whose contents describe nothing about a change: a dependency resolver's own record of
/// what it picked. Each is written by a tool, is often thousands of lines, and says only that
/// something else changed — which the change that caused it already says better.
const RESOLVED_NAMES: [&str; 6] = [
    "package-lock.json",
    "packages.lock.json",
    "pnpm-lock.yaml",
    "bun.lockb",
    "go.sum",
    "Package.resolved",
];

/// Suffixes of paths a tool wrote rather than a person. `.lock`/`.lockfile` covers the resolvers
/// that name their record after the manifest (`Cargo.lock`, `yarn.lock`, `Gemfile.lock`,
/// `poetry.lock`, `flake.lock`, `gradle.lockfile`); the rest are build output that mirrors a source
/// change already in the same commit.
const GENERATED_SUFFIXES: [&str; 5] = [".lock", ".lockfile", ".min.js", ".min.css", ".map"];

/// How many staged paths are described by their diff before the whole change is summarised instead.
///
/// Each one costs a read of the repository, so this bounds the work as well as the prompt: a change
/// touching hundreds of files is summarised rather than read hundreds of times, which is also the
/// better description of it.
const DESCRIBED_PATH_LIMIT: usize = 48;

/// Room kept back from the path summary so the line saying how many were left out always fits.
const REMAINDER_HEADROOM: usize = 64;

/// What a path is said to have had happen to it when nothing more precise applies — an unresolved
/// merge, or an untracked path, neither of which is a staged classification version control reports.
const CHANGED: &str = "changed";

impl Git {
    /// How the staged change is described: by its patches while they fit `budget`, otherwise by its
    /// paths.
    ///
    /// Reading stops as soon as the patches pass the budget, so the fallback costs at most the reads
    /// already made — and for a change over [`DESCRIBED_PATH_LIMIT`] paths, none at all.
    pub(super) fn describe(
        &self,
        project: ProjectId,
        root: &Path,
        staged: &[&FileChange],
        budget: usize,
    ) -> Result<Description, GitError> {
        if staged.len() > DESCRIBED_PATH_LIMIT {
            return Ok(Description::Paths(summarise(staged, budget)));
        }
        let mut patches = String::new();
        for change in staged {
            let Some(patch) = self.staged_patch(project, root, change)? else {
                continue;
            };
            if patches.len() + patch.len() > budget {
                return Ok(Description::Paths(summarise(staged, budget)));
            }
            patches.push_str(&patch);
        }
        // Every staged path was either binary or no longer differs from the last commit by the time
        // it was read, so there is no patch to show — the paths themselves are all that is left to
        // describe, and they still describe something.
        if patches.is_empty() {
            return Ok(Description::Paths(summarise(staged, budget)));
        }
        Ok(Description::Patches(patches))
    }

    /// One staged path's patch, or `None` when there is nothing in it to read: a path holding bytes
    /// rather than text, or one whose staged side turned out not to differ.
    fn staged_patch(
        &self,
        project: ProjectId,
        root: &Path,
        change: &FileChange,
    ) -> Result<Option<String>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        let raw = match self.repository.diff(
            root,
            DiffTarget::Staged,
            &change.path,
            change.original_path.as_deref(),
        ) {
            Ok(raw) => raw,
            Err(GitError::NotARepo) => return Ok(None),
            Err(err) => return Err(err),
        };
        if raw.binary || raw.hunks.is_empty() {
            return Ok(None);
        }
        let mut patch = raw.header;
        for hunk in &raw.hunks {
            patch.push_str(&hunk.text);
        }
        Ok(Some(patch))
    }
}

/// The form the staged change is described in.
pub(super) enum Description {
    /// The patches themselves, joined as version control produced them.
    Patches(String),
    /// One line per path: what happened to it, and where.
    Paths(String),
}

/// One line per staged path — what happened, then the path, and where it came from when it moved —
/// within `budget`, saying how many were left out when it runs out of room.
fn summarise(staged: &[&FileChange], budget: usize) -> String {
    let mut summary = String::new();
    let mut named = 0;
    for change in staged {
        let line = summary_line(change);
        if summary.len() + line.len() + REMAINDER_HEADROOM > budget {
            break;
        }
        summary.push_str(&line);
        named += 1;
    }
    let left_out = staged.len() - named;
    if left_out > 0 {
        summary.push_str(&format!("and {left_out} more files\n"));
    }
    summary
}

/// One path's line of the summary.
fn summary_line(change: &FileChange) -> String {
    let happened = change.status.staged.map_or(CHANGED, described);
    match &change.original_path {
        Some(from) => format!("{happened} {} (from {from})\n", change.path),
        None => format!("{happened} {}\n", change.path),
    }
}

/// What a staged path is said to have had happen to it. Words rather than version control's letters,
/// because this is read as prose by something that answers in prose.
fn described(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Modified => "modified",
        ChangeKind::TypeChanged => "changed type of",
        ChangeKind::Added => "added",
        ChangeKind::Deleted => "deleted",
        ChangeKind::Renamed => "renamed",
        ChangeKind::Copied => "copied",
        ChangeKind::Untracked | ChangeKind::Conflicted => CHANGED,
    }
}

/// Whether a path's contents say anything about the intent of the change that touched it.
pub(super) fn describes_intent(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    !RESOLVED_NAMES.contains(&name)
        && !GENERATED_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
}
