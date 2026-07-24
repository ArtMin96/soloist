//! The diagram aggregate (context C6): durable, project-scoped shared documents whose body is a raw
//! Mermaid **source string**.
//!
//! A diagram mirrors a [`scratchpad`](super::scratchpad) — durable, project-scoped, addressed by its
//! `name` handle, with revision-guarded writes — but its body is unconstrained Mermaid **source**
//! rather than Markdown. The core does **not** render or validate the Mermaid syntax; that is the
//! frontend's job. [`validate`] enforces only a non-blank `name` handle and the source size cap, so a
//! caller is free to write whatever diagram source the work needs. Writes are **revision-guarded**
//! (optimistic concurrency): a write carries the revision it expects, and a stale one is refused
//! rather than clobbering a newer edit. The durable [`DiagramRepo`](super::DiagramRepo) performs each
//! state-dependent step atomically.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::diagram_repo::{DiagramRepo, RenameResult, StoredDiagram, WriteResult};
use crate::ids::{DiagramId, ProjectId};
use crate::ports::{Clock, StoreError};

/// The most Mermaid source a diagram's body may carry, in bytes. A diagram is a coordination
/// document, not a log store; this bounds the persisted row so a runaway caller cannot grow the
/// table without limit. Mirrors the scratchpad cap — generous for a real diagram, far below the
/// transport frame ceiling.
pub const MAX_DIAGRAM_SOURCE_BYTES: usize = 256 * 1024;

/// Checks a diagram write is well-formed: the `name` handle is not blank and the `source` stays
/// within the size cap. The source may be blank — a blank document is valid; only the addressing
/// handle and the size ceiling are enforced. **The Mermaid syntax itself is never checked here** —
/// core does not render diagrams. Returns a single message naming every problem at once, or `Ok(())`
/// when it is well-formed.
fn validate(name: &str, source: &str) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    if name.trim().is_empty() {
        problems.push("name must not be blank".to_owned());
    }
    if source.len() > MAX_DIAGRAM_SOURCE_BYTES {
        problems.push(format!(
            "the source exceeds the {} KiB cap",
            MAX_DIAGRAM_SOURCE_BYTES / 1024
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// The one-line gist of a source for a listing: its first non-blank line, trimmed. For Mermaid this
/// is typically the diagram-kind directive (`flowchart TD`, `sequenceDiagram`), so the summary stays
/// a cheap scan key without embedding the whole source. A blank source has no gist (an empty string).
fn gist(source: &str) -> String {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_owned()
}

/// A reference to one diagram: the durable `id` that is stored and the `name` handle resolved when it
/// is read. Something that points *at* a diagram persists only the id, so a rename never breaks the
/// link; the handle is projected on read so a reader names the document instead of echoing a bare
/// number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramRef {
    pub id: DiagramId,
    pub name: String,
}

/// A diagram as a caller reads it: its durable identity and handle, its tags and archived flag, the
/// revision to guard the next write with, and the free-form Mermaid `source`. The source is stored
/// and returned verbatim — core never renders it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramView {
    pub id: DiagramId,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    pub source: String,
}

impl DiagramView {
    fn of(stored: StoredDiagram) -> Self {
        Self {
            id: stored.id,
            name: stored.name,
            tags: stored.tags,
            archived: stored.archived,
            revision: stored.revision,
            source: stored.source,
        }
    }
}

/// A diagram in a listing: its identity, handle, tags, archived flag, revision, and a one-line `gist`
/// of the source — enough to scan and pick one without fetching every document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramSummary {
    pub id: DiagramId,
    pub name: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    pub gist: String,
    /// Unix millis of the last source write (0 for a document that predates the field), so a listing
    /// can be sorted by recency as well as by name.
    pub updated_at: u64,
}

impl DiagramSummary {
    fn of(stored: StoredDiagram) -> Self {
        Self {
            gist: gist(&stored.source),
            id: stored.id,
            name: stored.name,
            tags: stored.tags,
            archived: stored.archived,
            revision: stored.revision,
            updated_at: stored.updated_at,
        }
    }
}

/// Why a [`write`](Diagrams::write) did not apply. Each is the caller's to fix: a malformed write, or
/// a revision that no longer matches (re-read and retry). A [`Store`](WriteError::Store) failure is
/// the server's.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// The write failed validation (a blank name or an over-cap source); the message names every
    /// problem.
    #[error("diagram is not well-formed: {0}")]
    Invalid(String),
    /// The write expected a different revision than the one on record — a concurrent edit landed
    /// first. `expected` is `None` for a create (the caller required absence); `actual` is `None`
    /// when no diagram exists under that name.
    #[error("diagram revision conflict (expected {expected:?}, found {actual:?})")]
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The diagram aggregate over the durable [`DiagramRepo`]. The repo persists and makes each
/// state-dependent step atomic; this aggregate owns the source validation and the revision-guard
/// policy. Cheap to clone-share via the `Arc` it holds.
pub struct Diagrams {
    repo: Arc<dyn DiagramRepo>,
    clock: Arc<dyn Clock>,
}

impl Diagrams {
    /// Builds the aggregate over its durable store and clock (the clock stamps each write's
    /// `updated_at`).
    pub fn new(repo: Arc<dyn DiagramRepo>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    /// Creates or replaces the diagram `name` in `project` with the Mermaid `source`,
    /// **revision-guarded**: `expected` is `None` to create (refused if one already exists) or the
    /// current revision to update (refused if it has since changed). On success returns the written
    /// diagram at its new revision; a stale write returns [`WriteError::Conflict`] and changes
    /// nothing, and a malformed write [`WriteError::Invalid`].
    pub fn write(
        &self,
        project: ProjectId,
        name: &str,
        source: String,
        expected: Option<u64>,
    ) -> Result<DiagramView, WriteError> {
        validate(name, &source).map_err(WriteError::Invalid)?;
        let now = self.clock.now_unix_millis();
        match self.repo.write(project, name, &source, expected, now)? {
            WriteResult::Written(stored) => Ok(DiagramView::of(*stored)),
            WriteResult::Conflict { actual } => Err(WriteError::Conflict { expected, actual }),
        }
    }

    /// The diagram `name` in `project`, or `None` if there is none.
    pub fn read(&self, project: ProjectId, name: &str) -> Result<Option<DiagramView>, StoreError> {
        Ok(self.repo.read(project, name)?.map(DiagramView::of))
    }

    /// Every diagram in `project` as a one-line summary, ordered by name.
    pub fn list(&self, project: ProjectId) -> Result<Vec<DiagramSummary>, StoreError> {
        Ok(self
            .repo
            .list(project)?
            .into_iter()
            .map(DiagramSummary::of)
            .collect())
    }

    /// Whether `project` owns the diagram `id` — the membership check a caller stating a reference by
    /// durable id is validated against, since an id names a row without naming a project.
    pub fn contains(&self, project: ProjectId, id: DiagramId) -> Result<bool, StoreError> {
        self.repo.contains(project, id)
    }

    /// Renames the diagram `from` to `to` in `project` (the durable id is unchanged), returning the
    /// renamed diagram. [`RenameError::NotFound`] if there is none, [`RenameError::NameTaken`] if `to`
    /// is already used in the project.
    pub fn rename(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<DiagramView, RenameError> {
        match self.repo.rename(project, from, to)? {
            RenameResult::Renamed(stored) => Ok(DiagramView::of(*stored)),
            RenameResult::NotFound => Err(RenameError::NotFound),
            RenameResult::NameTaken => Err(RenameError::NameTaken),
        }
    }

    /// Adds `tags` to the diagram `name` in `project` (idempotent — a tag already present is not
    /// duplicated), returning the updated diagram, or `None` if there is none.
    pub fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<DiagramView>, StoreError> {
        Ok(self
            .repo
            .add_tags(project, name, tags)?
            .map(DiagramView::of))
    }

    /// Removes `tags` from the diagram `name` in `project` (a tag not present is ignored), returning
    /// the updated diagram, or `None` if there is none.
    pub fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<DiagramView>, StoreError> {
        Ok(self
            .repo
            .remove_tags(project, name, tags)?
            .map(DiagramView::of))
    }

    /// The distinct tags used across `project`'s diagrams, sorted.
    pub fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        self.repo.tags(project)
    }

    /// Archives or restores the diagram `name` in `project`, returning the updated diagram, or `None`
    /// if there is none. Archiving keeps the source — it is a listing flag, not a delete.
    pub fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<DiagramView>, StoreError> {
        Ok(self
            .repo
            .set_archived(project, name, archived)?
            .map(DiagramView::of))
    }

    /// Deletes the diagram `name` in `project`, returning whether one was removed.
    pub fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        self.repo.delete(project, name)
    }
}

/// Why a [`rename`](Diagrams::rename) failed — both the caller's to fix.
#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    /// No diagram exists under the source name in the project.
    #[error("no diagram under that name")]
    NotFound,
    /// The target name is already used by another diagram in the project.
    #[error("a diagram with that name already exists")]
    NameTaken,
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[cfg(test)]
#[path = "diagram_tests.rs"]
mod tests;
