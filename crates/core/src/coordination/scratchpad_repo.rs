//! The durable store of coordination scratchpads and the port over it (context C6).
//!
//! Like the lease and timer ports, this is deliberately small and **atomic**: every operation that
//! depends on a scratchpad's current state — the revision-guarded [`write`](ScratchpadRepo::write),
//! the read-modify-write of tags, the rename's uniqueness check — is one indivisible method the
//! adapter performs under a single guard, never a read the aggregate then acts on, so two agents
//! editing one project's scratchpads cannot interleave to clobber an edit or duplicate a name. The
//! revision *guard* (a write applies only at the expected revision) is part of this contract; the
//! disciplined-document policy lives in the [`Scratchpads`](super::Scratchpads) aggregate. The
//! bounded context owns its own port (with a [`NoopScratchpadRepo`] default), so coordination
//! persistence stays confined to coordination. Unlike leases and timers, a scratchpad is durable
//! and **not** process-owned: there is no owner column and no launch-reconcile clear — it survives a
//! restart (matrix G11).

use super::scratchpad::ScratchpadDoc;
use crate::ids::{ProjectId, ScratchpadId};
use crate::ports::StoreError;

/// A persisted scratchpad: its store-assigned [`ScratchpadId`] (durable, stable across runs), the
/// project it belongs to, the mutable `name` handle, the disciplined document, its tags and
/// archived flag, and the `revision` the next write must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredScratchpad {
    pub id: ScratchpadId,
    pub project: ProjectId,
    pub name: String,
    pub doc: ScratchpadDoc,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
}

/// The outcome of a revision-guarded [`write`](ScratchpadRepo::write): either the write applied and
/// the stored row at its new revision is returned, or the expected revision no longer matched and
/// nothing changed. `actual` is the revision on record (`None` when no scratchpad exists under that
/// name), which the aggregate pairs with the caller's expectation to report the conflict. The stored
/// row is boxed so the common small `Conflict` variant does not inflate the enum to its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteResult {
    Written(Box<StoredScratchpad>),
    Conflict { actual: Option<u64> },
}

/// The outcome of a [`rename`](ScratchpadRepo::rename): the renamed row, no scratchpad under the
/// source name, or the target name already in use within the project. The stored row is boxed so the
/// small `NotFound`/`NameTaken` variants do not inflate the enum to its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameResult {
    Renamed(Box<StoredScratchpad>),
    NotFound,
    NameTaken,
}

/// Durable repository of coordination scratchpads. One row per `(project, name)`, keyed for
/// addressing by `name` but identified durably by a store-assigned [`ScratchpadId`] a rename does
/// not change. Every state-dependent method is atomic with respect to the others.
pub trait ScratchpadRepo: Send + Sync {
    /// Creates or updates the scratchpad `(project, name)` with `doc`, **revision-guarded** in one
    /// atomic step: `expected` is `None` to create (applies only if absent) or the current revision
    /// to update (applies only if it still matches). On success returns
    /// [`WriteResult::Written`] with the stored row at its new revision (1 on create, otherwise
    /// `expected + 1`); on a mismatch returns [`WriteResult::Conflict`] with the revision on record
    /// and changes nothing.
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        doc: &ScratchpadDoc,
        expected: Option<u64>,
    ) -> Result<WriteResult, StoreError>;

    /// The scratchpad `(project, name)`, or `None` if there is none.
    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredScratchpad>, StoreError>;

    /// Every scratchpad in `project`, ordered by name.
    fn list(&self, project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError>;

    /// Renames `(project, from)` to `to` (the durable id is unchanged), checking target uniqueness
    /// as part of the update so two renames cannot both take one name.
    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError>;

    /// Adds `tags` to `(project, name)` — read-modify-write of the tag set under one guard, so a
    /// concurrent tag change is not lost. Idempotent. Returns the updated row, or `None` if absent.
    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError>;

    /// Removes `tags` from `(project, name)` — read-modify-write under one guard. A tag not present
    /// is ignored. Returns the updated row, or `None` if absent.
    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError>;

    /// The distinct tags used across `project`'s scratchpads, sorted.
    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError>;

    /// Sets the archived flag of `(project, name)`, returning the updated row, or `None` if absent.
    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredScratchpad>, StoreError>;

    /// Deletes `(project, name)`, returning whether one was removed.
    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError>;
}

/// A [`ScratchpadRepo`] that stores nothing — the default until the durable adapter is wired, so the
/// core runs (scratchpads simply never persist) without it. A write echoes a placeholder row back
/// and every read is empty.
#[derive(Clone, Copy, Default)]
pub struct NoopScratchpadRepo;

impl ScratchpadRepo for NoopScratchpadRepo {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        doc: &ScratchpadDoc,
        _expected: Option<u64>,
    ) -> Result<WriteResult, StoreError> {
        Ok(WriteResult::Written(Box::new(StoredScratchpad {
            id: ScratchpadId::from_raw(0),
            project,
            name: name.to_owned(),
            doc: doc.clone(),
            tags: Vec::new(),
            archived: false,
            revision: 1,
        })))
    }
    fn read(
        &self,
        _project: ProjectId,
        _name: &str,
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        Ok(None)
    }
    fn list(&self, _project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError> {
        Ok(Vec::new())
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
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        Ok(None)
    }
    fn remove_tags(
        &self,
        _project: ProjectId,
        _name: &str,
        _tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
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
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        Ok(None)
    }
    fn delete(&self, _project: ProjectId, _name: &str) -> Result<bool, StoreError> {
        Ok(false)
    }
}
