//! The scratchpad aggregate (context C6): durable, project-scoped shared documents agents
//! coordinate through.
//!
//! Unlike a lease or a timer, a scratchpad is **not** process-owned and is **durable** — it
//! survives an app restart, so launch reconciliation never clears it. A scratchpad is a **free-form
//! Markdown note** addressed by its `name` handle: the `name` is the document's identity and is not
//! duplicated inside the body. The body is unconstrained Markdown (bounded only by a size cap), so a
//! caller is free to shape it — a template seeds the initial content, but the schema is not enforced.
//! Writes are **revision-guarded** (optimistic concurrency): a write carries the revision it expects,
//! and a stale one is refused rather than clobbering a newer edit. The durable
//! [`ScratchpadRepo`](super::ScratchpadRepo) performs each state-dependent step atomically.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::scratchpad_repo::{
    RenameResult, ScratchpadRepo, StoredScratchpad, TransferResult, WriteResult,
};
use crate::ids::{ProjectId, ScratchpadId, TodoId};
use crate::ports::{Clock, StoreError};

/// The most Markdown a scratchpad's body may carry, in bytes. A scratchpad is a coordination
/// document, not a log store; this bounds the persisted row so a runaway caller cannot grow the
/// table without limit. Generous for a real note, far below the transport frame ceiling.
pub const MAX_SCRATCHPAD_CONTENT_BYTES: usize = 256 * 1024;

/// Checks a scratchpad write is well-formed: the `name` handle is not blank and the `body` stays
/// within the size cap. The body may be blank — a blank document is valid; only the addressing
/// handle and the size ceiling are enforced. Returns a single message naming every problem at once,
/// or `Ok(())` when it is well-formed.
fn validate(name: &str, body: &str) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    if name.trim().is_empty() {
        problems.push("name must not be blank".to_owned());
    }
    if body.len() > MAX_SCRATCHPAD_CONTENT_BYTES {
        problems.push(format!(
            "the content exceeds the {} KiB cap",
            MAX_SCRATCHPAD_CONTENT_BYTES / 1024
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Renders a scratchpad for export/read: the `name` as the leading H1 over its Markdown body. The
/// single rendering used everywhere a scratchpad is read whole, so every reader — agent or UI —
/// sees the same shape, and the name (the identity) is never stored inside the body.
fn render(name: &str, body: &str) -> String {
    let body = body.trim_end();
    if body.is_empty() {
        format!("# {name}\n")
    } else {
        format!("# {name}\n\n{body}\n")
    }
}

/// The one-line gist of a body for a listing: its first non-blank, non-heading line, trimmed. A body
/// that is empty or only headings has no gist (an empty string), so the summary stays a cheap scan
/// key without embedding the whole note.
fn gist(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or("")
        .to_owned()
}

/// A reference to one scratchpad: the durable `id` that is stored and the `name` handle resolved
/// when it is read. Something that points *at* a scratchpad — a todo's association — persists only
/// the id, so a rename never breaks the link; the handle is projected on read so a reader names the
/// document instead of echoing a bare number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchpadRef {
    pub id: ScratchpadId,
    pub name: String,
}

/// A scratchpad as a caller reads it: its durable identity and handle, its tags and archived flag,
/// the revision to guard the next write with, the free-form Markdown `body`, and that body rendered
/// under its name as canonical Markdown. The `rendered` text is derived (not stored), so the
/// persisted shape stays the body alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchpadView {
    pub id: ScratchpadId,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    pub body: String,
    pub rendered: String,
}

impl ScratchpadView {
    fn of(stored: StoredScratchpad) -> Self {
        let rendered = render(&stored.name, &stored.body);
        Self {
            id: stored.id,
            name: stored.name,
            tags: stored.tags,
            archived: stored.archived,
            revision: stored.revision,
            body: stored.body,
            rendered,
        }
    }
}

/// A scratchpad in a listing: its identity, handle, tags, archived flag, revision, and a one-line
/// `gist` of the body — enough to scan and pick one without fetching every document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchpadSummary {
    pub id: ScratchpadId,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    pub gist: String,
    /// Unix millis of the last body write (0 for a document that predates the field), so a listing
    /// can be sorted by recency as well as by name.
    pub updated_at: u64,
}

impl ScratchpadSummary {
    fn of(stored: StoredScratchpad) -> Self {
        Self {
            gist: gist(&stored.body),
            id: stored.id,
            name: stored.name,
            tags: stored.tags,
            archived: stored.archived,
            revision: stored.revision,
            updated_at: stored.updated_at,
        }
    }
}

/// Why a [`write`](Scratchpads::write) did not apply. Each is the caller's to fix: a malformed
/// write, or a revision that no longer matches (re-read and retry). A [`Store`](WriteError::Store)
/// failure is the server's.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// The write failed validation (a blank name or an over-cap body); the message names every problem.
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
/// state-dependent step atomic; this aggregate owns the body validation and the revision-guard
/// policy. Cheap to clone-share via the `Arc` it holds.
pub struct Scratchpads {
    repo: Arc<dyn ScratchpadRepo>,
    clock: Arc<dyn Clock>,
}

impl Scratchpads {
    /// Builds the aggregate over its durable store and clock (the clock stamps each write's
    /// `updated_at`).
    pub fn new(repo: Arc<dyn ScratchpadRepo>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    /// Creates or replaces the scratchpad `name` in `project` with the Markdown `body`,
    /// **revision-guarded**: `expected` is `None` to create (refused if one already exists) or the
    /// current revision to update (refused if it has since changed). On success returns the written
    /// scratchpad at its new revision; a stale write returns [`WriteError::Conflict`] and changes
    /// nothing, and a malformed write [`WriteError::Invalid`].
    pub fn write(
        &self,
        project: ProjectId,
        name: &str,
        body: String,
        expected: Option<u64>,
    ) -> Result<ScratchpadView, WriteError> {
        validate(name, &body).map_err(WriteError::Invalid)?;
        let now = self.clock.now_unix_millis();
        match self.repo.write(project, name, &body, expected, now)? {
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

    /// Moves the scratchpad `name` from `from` to `to`, keeping its name, body, tags, archived
    /// flag, revision, and durable id, and **taking the todos derived from it along** with their
    /// association intact (see [`ScratchpadRepo::transfer`] for the full move contract).
    /// [`RenameError::NotFound`] if `from` has no such scratchpad, [`RenameError::NameTaken`] if
    /// `to` already has one under that name (reusing the rename error taxonomy — a transfer is a
    /// cross-project relocation with the same two failure modes).
    pub fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<ScratchpadTransfer, RenameError> {
        match self.repo.transfer(from, name, to)? {
            TransferResult::Transferred(moved) => Ok(ScratchpadTransfer {
                scratchpad: ScratchpadView::of(moved.scratchpad),
                todos: moved.todos,
            }),
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
    /// `None` if there is none. Archiving keeps the body — it is a listing flag, not a delete.
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

/// A completed [`transfer`](Scratchpads::transfer): the scratchpad as it now reads in the target
/// project, and the todos that derived from it and moved with it. The caller announces the move on
/// both boards from these two, so a to-do board never keeps showing work that has left.
pub struct ScratchpadTransfer {
    pub scratchpad: ScratchpadView,
    pub todos: Vec<TodoId>,
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
