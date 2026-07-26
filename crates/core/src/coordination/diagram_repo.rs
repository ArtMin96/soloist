//! The durable store of coordination diagrams and the port over it (context C6).
//!
//! Like the scratchpad port, this is deliberately small and **atomic**: every operation that depends
//! on a diagram's current state — the revision-guarded [`write`](DiagramRepo::write), the
//! read-modify-write of tags, the rename's uniqueness check — is one indivisible method the adapter
//! performs under a single guard, never a read the aggregate then acts on, so two agents editing one
//! project's diagrams cannot interleave to clobber an edit or duplicate a name. The revision *guard*
//! (a write applies only at the expected revision) is part of this contract; the source validation
//! lives in the [`Diagrams`](super::Diagrams) aggregate. The bounded context owns its own port (with
//! a [`NoopDiagramRepo`] default), so coordination persistence stays confined to coordination. Like a
//! scratchpad, a diagram is durable and **not** process-owned: there is no owner column and no
//! launch-reconcile clear — it survives an app restart.

use crate::ids::{DiagramId, ProjectId};
use crate::ports::StoreError;

/// A persisted diagram: its store-assigned [`DiagramId`] (durable, stable across runs), the project
/// it belongs to, the mutable `name` handle, the free-form Mermaid `source`, its tags and archived
/// flag, and the `revision` the next write must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredDiagram {
    pub id: DiagramId,
    pub project: ProjectId,
    pub name: String,
    pub source: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    /// Unix millis of the last source write (0 for a row that predates the field). The recency key
    /// the listing sorts on; unaffected by archive/rename/tag changes, which are not source edits.
    pub updated_at: u64,
}

/// The outcome of a revision-guarded [`write`](DiagramRepo::write): either the write applied and the
/// stored row at its new revision is returned, or the expected revision no longer matched and nothing
/// changed. `actual` is the revision on record (`None` when no diagram exists under that name), which
/// the aggregate pairs with the caller's expectation to report the conflict. The stored row is boxed
/// so the common small `Conflict` variant does not inflate the enum to its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteResult {
    Written(Box<StoredDiagram>),
    Conflict { actual: Option<u64> },
}

/// The outcome of a [`rename`](DiagramRepo::rename): the renamed row, no diagram under the source
/// name, or the target name already in use within the project. The stored row is boxed so the small
/// `NotFound`/`NameTaken` variants do not inflate the enum to its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameResult {
    Renamed(Box<StoredDiagram>),
    NotFound,
    NameTaken,
}

/// Durable repository of coordination diagrams. One row per `(project, name)`, keyed for addressing
/// by `name` but identified durably by a store-assigned [`DiagramId`] a rename does not change. Every
/// state-dependent method is atomic with respect to the others.
pub trait DiagramRepo: Send + Sync {
    /// Creates or updates the diagram `(project, name)` with `source`, **revision-guarded** in one
    /// atomic step: `expected` is `None` to create (applies only if absent) or the current revision
    /// to update (applies only if it still matches). On success returns [`WriteResult::Written`] with
    /// the stored row at its new revision (1 on create, otherwise `expected + 1`); on a mismatch
    /// returns [`WriteResult::Conflict`] with the revision on record and changes nothing. `now` is the
    /// wall-clock (unix millis) the applied write stamps `updated_at` with — supplied by the
    /// aggregate's [`Clock`](crate::ports::Clock) so the store stays deterministic under test.
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        source: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<WriteResult, StoreError>;

    /// The diagram `(project, name)`, or `None` if there is none.
    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredDiagram>, StoreError>;

    /// Every diagram in `project`, ordered by name.
    fn list(&self, project: ProjectId) -> Result<Vec<StoredDiagram>, StoreError>;

    /// Whether `project` owns the diagram `id`. A [`DiagramId`] addresses a row directly and so
    /// carries no project with it: a caller that states a reference by durable id — rather than by a
    /// name resolved inside a project — has to be checked against the project first.
    fn contains(&self, project: ProjectId, id: DiagramId) -> Result<bool, StoreError>;

    /// Renames `(project, from)` to `to` (the durable id is unchanged), checking target uniqueness as
    /// part of the update so two renames cannot both take one name.
    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError>;

    /// Adds `tags` to `(project, name)` — read-modify-write of the tag set under one guard, so a
    /// concurrent tag change is not lost. Idempotent. Returns the updated row, or `None` if absent.
    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError>;

    /// Removes `tags` from `(project, name)` — read-modify-write under one guard. A tag not present is
    /// ignored. Returns the updated row, or `None` if absent.
    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError>;

    /// The distinct tags used across `project`'s diagrams, sorted.
    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError>;

    /// Sets the archived flag of `(project, name)`, returning the updated row, or `None` if absent.
    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredDiagram>, StoreError>;

    /// Deletes `(project, name)`, returning whether one was removed.
    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError>;
}

/// A [`DiagramRepo`] that stores nothing — the default until the durable adapter is wired, so the
/// core runs (diagrams simply never persist) without it. A write echoes a placeholder row back and
/// every read is empty.
#[derive(Clone, Copy, Default)]
pub struct NoopDiagramRepo;

impl DiagramRepo for NoopDiagramRepo {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        source: &str,
        _expected: Option<u64>,
        now: u64,
    ) -> Result<WriteResult, StoreError> {
        Ok(WriteResult::Written(Box::new(StoredDiagram {
            id: DiagramId::from_raw(0),
            project,
            name: name.to_owned(),
            source: source.to_owned(),
            tags: Vec::new(),
            archived: false,
            revision: 1,
            updated_at: now,
        })))
    }
    fn read(&self, _project: ProjectId, _name: &str) -> Result<Option<StoredDiagram>, StoreError> {
        Ok(None)
    }
    fn list(&self, _project: ProjectId) -> Result<Vec<StoredDiagram>, StoreError> {
        Ok(Vec::new())
    }
    fn contains(&self, _project: ProjectId, _id: DiagramId) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn rename(
        &self,
        _project: ProjectId,
        _from: &str,
        _to: &str,
    ) -> Result<RenameResult, StoreError> {
        Ok(RenameResult::NotFound)
    }
    fn add_tags(
        &self,
        _project: ProjectId,
        _name: &str,
        _tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError> {
        Ok(None)
    }
    fn remove_tags(
        &self,
        _project: ProjectId,
        _name: &str,
        _tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError> {
        Ok(None)
    }
    fn tags(&self, _project: ProjectId) -> Result<Vec<String>, StoreError> {
        Ok(Vec::new())
    }
    fn set_archived(
        &self,
        _project: ProjectId,
        _name: &str,
        _archived: bool,
    ) -> Result<Option<StoredDiagram>, StoreError> {
        Ok(None)
    }
    fn delete(&self, _project: ProjectId, _name: &str) -> Result<bool, StoreError> {
        Ok(false)
    }
}
