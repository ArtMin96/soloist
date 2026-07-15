//! The scratchpad aggregate (context C6): durable, project-scoped shared documents agents
//! coordinate through.
//!
//! Unlike a lease or a timer, a scratchpad is **not** process-owned and is **durable** — it
//! survives an app restart, so launch reconciliation never clears it. A scratchpad
//! carries a **disciplined, typed body** ([`ScratchpadDoc`]): objective, context, an ordered plan,
//! acceptance criteria, risks, and a status, plus optional free notes. The shape is enforced (the
//! tool schema presents exactly these fields and the aggregate rejects a blank one), so every agent
//! writes the same informative structure rather than free-form prose, and the document renders to
//! one canonical Markdown layout. Writes are **revision-guarded** (optimistic concurrency): a write
//! carries the revision it expects, and a stale one is refused rather than clobbering a newer edit.
//! The durable [`ScratchpadRepo`](super::ScratchpadRepo) performs each state-dependent step
//! atomically.

use std::fmt::Write as _;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::scratchpad_repo::{
    RenameResult, ScratchpadRepo, StoredScratchpad, TransferResult, WriteResult,
};
use crate::ids::{ProjectId, ScratchpadId};
use crate::ports::StoreError;

/// The most text content a scratchpad may carry, summed across its sections, in bytes. A
/// scratchpad is a coordination document, not a log store; this bounds the persisted row so a
/// runaway caller cannot grow the table without limit. Generous for a real plan, far below the
/// transport frame ceiling.
pub const MAX_SCRATCHPAD_CONTENT_BYTES: usize = 256 * 1024;

/// The disciplined body every scratchpad carries. The fields are a fixed, ordered structure — what
/// makes a scratchpad a consistent, informative coordination artifact rather than free-form notes:
/// the objective it serves, the context behind it, the ordered plan (the path), the acceptance
/// criteria that define done, the risks to watch, and a status line. `notes` is the one open field
/// for anything the structure does not cover. The aggregate validates the structure on write
/// ([`validate`](ScratchpadDoc::validate)) and renders it to one canonical Markdown layout
/// ([`render`](ScratchpadDoc::render)), so the shape and its presentation are single-source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchpadDoc {
    /// What this scratchpad is for — the goal it serves, in a sentence or two.
    pub objective: String,
    /// The background and current state a reader needs to act on it.
    pub context: String,
    /// The ordered path to the objective: each entry one step, in order.
    pub plan: Vec<String>,
    /// The testable criteria that define the objective as done.
    pub acceptance_criteria: Vec<String>,
    /// The risks, unknowns, or blockers to watch — state "none identified" rather than omitting.
    pub risks: Vec<String>,
    /// The current progress: where the work stands right now.
    pub status: String,
    /// Anything the structured sections do not cover — free Markdown, optional.
    pub notes: Option<String>,
}

impl ScratchpadDoc {
    /// Checks the disciplined structure is present and informative: no required field is blank, and
    /// each list has at least one non-blank entry. Returns a single message naming **every** problem
    /// at once (so an agent fixes the document in one revision), or `Ok(())` when it is well-formed.
    /// `notes` is optional and unconstrained.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems: Vec<&str> = Vec::new();
        if self.objective.trim().is_empty() {
            problems.push("objective must not be blank");
        }
        if self.context.trim().is_empty() {
            problems.push("context must not be blank");
        }
        if self.status.trim().is_empty() {
            problems.push("status must not be blank");
        }
        check_list(
            &self.plan,
            "plan needs at least one step",
            "plan steps must not be blank",
            &mut problems,
        );
        check_list(
            &self.acceptance_criteria,
            "acceptance_criteria needs at least one criterion",
            "acceptance_criteria entries must not be blank",
            &mut problems,
        );
        check_list(
            &self.risks,
            "risks needs at least one entry",
            "risks entries must not be blank",
            &mut problems,
        );
        let mut messages: Vec<String> = problems
            .iter()
            .map(|problem| (*problem).to_owned())
            .collect();
        if self.content_bytes() > MAX_SCRATCHPAD_CONTENT_BYTES {
            messages.push(format!(
                "the content exceeds the {} KiB cap",
                MAX_SCRATCHPAD_CONTENT_BYTES / 1024
            ));
        }
        if messages.is_empty() {
            Ok(())
        } else {
            Err(messages.join("; "))
        }
    }

    /// The total bytes of the document's text across every section — what a size cap bounds.
    fn content_bytes(&self) -> usize {
        self.objective.len()
            + self.context.len()
            + self.status.len()
            + self.notes.as_deref().map_or(0, str::len)
            + self.plan.iter().map(String::len).sum::<usize>()
            + self
                .acceptance_criteria
                .iter()
                .map(String::len)
                .sum::<usize>()
            + self.risks.iter().map(String::len).sum::<usize>()
    }

    /// Renders the document to its one canonical Markdown layout, titled by the scratchpad's `name`
    /// (the leading H1). The single rendering used everywhere a scratchpad is read, so every reader
    /// — agent or the future UI — sees the same shape.
    pub fn render(&self, name: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# {name}\n");
        let _ = writeln!(out, "## Objective\n{}\n", self.objective.trim());
        let _ = writeln!(out, "## Context\n{}\n", self.context.trim());
        let _ = writeln!(out, "## Plan");
        for (index, step) in self.plan.iter().enumerate() {
            let _ = writeln!(out, "{}. {}", index + 1, step.trim());
        }
        let _ = writeln!(out, "\n## Acceptance criteria");
        for criterion in &self.acceptance_criteria {
            let _ = writeln!(out, "- [ ] {}", criterion.trim());
        }
        let _ = writeln!(out, "\n## Risks");
        for risk in &self.risks {
            let _ = writeln!(out, "- {}", risk.trim());
        }
        let _ = writeln!(out, "\n## Status\n{}", self.status.trim());
        if let Some(notes) = self
            .notes
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            let _ = writeln!(out, "\n## Notes\n{notes}");
        }
        out
    }
}

/// Records a list-section problem: `if_empty` when it carries nothing (empty, or every entry
/// blank), or `if_blank` when it has a blank entry among real ones.
fn check_list<'a>(
    items: &[String],
    if_empty: &'a str,
    if_blank: &'a str,
    problems: &mut Vec<&'a str>,
) {
    if items.iter().all(|item| item.trim().is_empty()) {
        problems.push(if_empty);
    } else if items.iter().any(|item| item.trim().is_empty()) {
        problems.push(if_blank);
    }
}

/// A scratchpad as a caller reads it: its durable identity and handle, its tags and archived flag,
/// the revision to guard the next write with, the disciplined [`ScratchpadDoc`], and that document
/// rendered to canonical Markdown. The `rendered` text is derived (not stored), so the persisted
/// shape stays the structured document alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchpadView {
    pub id: ScratchpadId,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    pub doc: ScratchpadDoc,
    pub rendered: String,
}

impl ScratchpadView {
    fn of(stored: StoredScratchpad) -> Self {
        let rendered = stored.doc.render(&stored.name);
        Self {
            id: stored.id,
            name: stored.name,
            tags: stored.tags,
            archived: stored.archived,
            revision: stored.revision,
            doc: stored.doc,
            rendered,
        }
    }
}

/// A scratchpad in a listing: its identity, handle, tags, archived flag, revision, and the
/// objective as a one-line gist — enough to scan and pick one without fetching every document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchpadSummary {
    pub id: ScratchpadId,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    pub objective: String,
}

impl ScratchpadSummary {
    fn of(stored: StoredScratchpad) -> Self {
        Self {
            id: stored.id,
            name: stored.name,
            tags: stored.tags,
            archived: stored.archived,
            revision: stored.revision,
            objective: stored.doc.objective,
        }
    }
}

/// Why a [`write`](Scratchpads::write) did not apply. Each is the caller's to fix: a malformed
/// document, or a revision that no longer matches (re-read and retry). A [`Store`](WriteError::Store)
/// failure is the server's.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// The document failed the disciplined-structure check; the message names every problem.
    #[error("scratchpad is not well-formed: {0}")]
    Invalid(String),
    /// The write expected a different revision than the one on record — a concurrent edit landed
    /// first. `expected` is `None` for a create (the caller required absence); `actual` is `None`
    /// when no scratchpad exists under that name.
    #[error("scratchpad revision conflict (expected {expected:?}, found {actual:?})")]
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The scratchpad aggregate over the durable [`ScratchpadRepo`]. The repo persists and makes each
/// state-dependent step atomic; this aggregate owns the disciplined-document validation and the
/// revision-guard policy. Cheap to clone-share via the `Arc` it holds.
pub struct Scratchpads {
    repo: Arc<dyn ScratchpadRepo>,
}

impl Scratchpads {
    /// Builds the aggregate over its durable store.
    pub fn new(repo: Arc<dyn ScratchpadRepo>) -> Self {
        Self { repo }
    }

    /// Creates or replaces the scratchpad `name` in `project` with the disciplined `doc`,
    /// **revision-guarded**: `expected` is `None` to create (refused if one already exists) or the
    /// current revision to update (refused if it has since changed). On success returns the written
    /// scratchpad at its new revision; a stale write returns [`WriteError::Conflict`] and changes
    /// nothing, and a malformed document [`WriteError::Invalid`].
    pub fn write(
        &self,
        project: ProjectId,
        name: &str,
        doc: ScratchpadDoc,
        expected: Option<u64>,
    ) -> Result<ScratchpadView, WriteError> {
        doc.validate().map_err(WriteError::Invalid)?;
        match self.repo.write(project, name, &doc, expected)? {
            WriteResult::Written(stored) => Ok(ScratchpadView::of(*stored)),
            WriteResult::Conflict { actual } => Err(WriteError::Conflict { expected, actual }),
        }
    }

    /// The scratchpad `name` in `project`, or `None` if there is none.
    pub fn read(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<Option<ScratchpadView>, StoreError> {
        Ok(self.repo.read(project, name)?.map(ScratchpadView::of))
    }

    /// Every scratchpad in `project` as a one-line summary, ordered by name.
    pub fn list(&self, project: ProjectId) -> Result<Vec<ScratchpadSummary>, StoreError> {
        Ok(self
            .repo
            .list(project)?
            .into_iter()
            .map(ScratchpadSummary::of)
            .collect())
    }

    /// Renames the scratchpad `from` to `to` in `project` (the durable id is unchanged), returning
    /// the renamed scratchpad. [`RenameError::NotFound`] if there is none, [`RenameError::NameTaken`]
    /// if `to` is already used in the project.
    pub fn rename(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<ScratchpadView, RenameError> {
        match self.repo.rename(project, from, to)? {
            RenameResult::Renamed(stored) => Ok(ScratchpadView::of(*stored)),
            RenameResult::NotFound => Err(RenameError::NotFound),
            RenameResult::NameTaken => Err(RenameError::NameTaken),
        }
    }

    /// Moves the scratchpad `name` from `from` to `to`, keeping its name, document, tags, archived
    /// flag, revision, and durable id. [`RenameError::NotFound`] if `from` has no such scratchpad,
    /// [`RenameError::NameTaken`] if `to` already has one under that name (reusing the rename error
    /// taxonomy — a transfer is a cross-project relocation with the same two failure modes).
    pub fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<ScratchpadView, RenameError> {
        match self.repo.transfer(from, name, to)? {
            TransferResult::Transferred(stored) => Ok(ScratchpadView::of(*stored)),
            TransferResult::NotFound => Err(RenameError::NotFound),
            TransferResult::NameTaken => Err(RenameError::NameTaken),
        }
    }

    /// Adds `tags` to the scratchpad `name` in `project` (idempotent — a tag already present is not
    /// duplicated), returning the updated scratchpad, or `None` if there is none.
    pub fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<ScratchpadView>, StoreError> {
        Ok(self
            .repo
            .add_tags(project, name, tags)?
            .map(ScratchpadView::of))
    }

    /// Removes `tags` from the scratchpad `name` in `project` (a tag not present is ignored),
    /// returning the updated scratchpad, or `None` if there is none.
    pub fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<ScratchpadView>, StoreError> {
        Ok(self
            .repo
            .remove_tags(project, name, tags)?
            .map(ScratchpadView::of))
    }

    /// The distinct tags used across `project`'s scratchpads, sorted.
    pub fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        self.repo.tags(project)
    }

    /// Archives or restores the scratchpad `name` in `project`, returning the updated scratchpad, or
    /// `None` if there is none. Archiving keeps the document — it is a listing flag, not a delete.
    pub fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<ScratchpadView>, StoreError> {
        Ok(self
            .repo
            .set_archived(project, name, archived)?
            .map(ScratchpadView::of))
    }

    /// Deletes the scratchpad `name` in `project`, returning whether one was removed.
    pub fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        self.repo.delete(project, name)
    }
}

/// Why a [`rename`](Scratchpads::rename) failed — both the caller's to fix.
#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    /// No scratchpad exists under the source name in the project.
    #[error("no scratchpad under that name")]
    NotFound,
    /// The target name is already used by another scratchpad in the project.
    #[error("a scratchpad with that name already exists")]
    NameTaken,
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[cfg(test)]
#[path = "scratchpad_tests.rs"]
mod tests;
