//! The durable store of coordination scratchpads and the port over it (context C6).
//!
//! Like the lease and timer ports, this is deliberately small and **atomic**: every operation that
//! depends on a scratchpad's current state — the revision-guarded [`write`](ScratchpadRepo::write),
//! the read-modify-write of tags, the rename's uniqueness check — is one indivisible method the
//! adapter performs under a single guard, never a read the aggregate then acts on, so two agents
//! editing one project's scratchpads cannot interleave to clobber an edit or duplicate a name. The
//! revision *guard* (a write applies only at the expected revision) is part of this contract; the
//! body validation lives in the [`Scratchpads`](super::Scratchpads) aggregate. The bounded context
//! owns its own port (with a [`NoopScratchpadRepo`] default), so coordination persistence stays
//! confined to coordination. Unlike leases and timers, a scratchpad is durable and **not**
//! process-owned: there is no owner column and no launch-reconcile clear — it survives an app
//! restart.

use crate::ids::{ProjectId, ScratchpadId, TodoId};
use crate::ports::StoreError;

/// A persisted scratchpad: its store-assigned [`ScratchpadId`] (durable, stable across runs), the
/// project it belongs to, the mutable `name` handle, the free-form Markdown `body`, its tags and
/// archived flag, and the `revision` the next write must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredScratchpad {
    pub id: ScratchpadId,
    pub project: ProjectId,
    pub name: String,
    pub body: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub revision: u64,
    /// Unix millis of the last body write (0 for a row that predates the field). The recency key the
    /// listing sorts on; unaffected by archive/rename/tag changes, which are not body edits.
    pub updated_at: u64,
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

/// A completed cross-project [`transfer`](ScratchpadRepo::transfer): the scratchpad now under the
/// target project, and the todos that were derived from it and moved along with it. Both ends of
/// each association moved, so every listed todo still points at this scratchpad.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferredScratchpad {
    pub scratchpad: StoredScratchpad,
    /// The derived todos that moved, in id order. Empty when nothing derived from the scratchpad.
    pub todos: Vec<TodoId>,
}

/// The outcome of a cross-project [`transfer`](ScratchpadRepo::transfer): the completed move, no
/// scratchpad under the source name, or the target project already using that name. The success
/// payload is boxed so the small miss variants stay small.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferResult {
    Transferred(Box<TransferredScratchpad>),
    NotFound,
    NameTaken,
}

/// Durable repository of coordination scratchpads. One row per `(project, name)`, keyed for
/// addressing by `name` but identified durably by a store-assigned [`ScratchpadId`] a rename does
/// not change. Every state-dependent method is atomic with respect to the others.
pub trait ScratchpadRepo: Send + Sync {
    /// Creates or updates the scratchpad `(project, name)` with `body`, **revision-guarded** in one
    /// atomic step: `expected` is `None` to create (applies only if absent) or the current revision
    /// to update (applies only if it still matches). On success returns
    /// [`WriteResult::Written`] with the stored row at its new revision (1 on create, otherwise
    /// `expected + 1`); on a mismatch returns [`WriteResult::Conflict`] with the revision on record
    /// and changes nothing. `now` is the wall-clock (unix millis) the applied write stamps
    /// `updated_at` with — supplied by the aggregate's [`Clock`](crate::ports::Clock) so the store
    /// stays deterministic under test.
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        body: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<WriteResult, StoreError>;

    /// The scratchpad `(project, name)`, or `None` if there is none.
    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredScratchpad>, StoreError>;

    /// Every scratchpad in `project`, ordered by name.
    fn list(&self, project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError>;

    /// Renames `(project, from)` to `to` (the durable id is unchanged), checking target uniqueness
    /// as part of the update so two renames cannot both take one name.
    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError>;

    /// Moves the scratchpad `(from, name)` to project `to`, keeping its `name`, document, tags,
    /// archived flag, revision, and durable id. Checks the target `(to, name)` is free as part of
    /// the update — a clearer [`TransferResult::NameTaken`] than a UNIQUE violation — so a move
    /// cannot collide with an existing scratchpad.
    ///
    /// **The todos derived from the scratchpad move with it**, since a todo's association records
    /// that it came out of that document: their link is **kept** (both ends move, so it stays
    /// valid — unlike [`TodoRepo::transfer`](super::TodoRepo::transfer), which clears it because
    /// the scratchpad stays behind), their process-owned lock is dropped, and a blocker naming a
    /// todo left behind is dropped while one naming a todo that moves too survives. Todos in `from`
    /// that derive from no scratchpad, or from another one, are untouched.
    ///
    /// One atomic step: the document and its derived todos either all move or none do, so no read
    /// ever sees a todo stranded from the scratchpad it derives from.
    fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<TransferResult, StoreError>;

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

    /// Deletes `(project, name)`, returning whether one was removed. Any todo associated with the
    /// removed scratchpad is left **unlinked** rather than pointing at a document that is gone —
    /// part of this contract, not the caller's to clean up afterwards.
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
        body: &str,
        _expected: Option<u64>,
        now: u64,
    ) -> Result<WriteResult, StoreError> {
        Ok(WriteResult::Written(Box::new(StoredScratchpad {
            id: ScratchpadId::from_raw(0),
            project,
            name: name.to_owned(),
            body: body.to_owned(),
            tags: Vec::new(),
            archived: false,
            revision: 1,
            updated_at: now,
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
    fn transfer(
        &self,
        _from: ProjectId,
        _name: &str,
        _to: ProjectId,
    ) -> Result<TransferResult, StoreError> {
        Ok(TransferResult::NotFound)
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
