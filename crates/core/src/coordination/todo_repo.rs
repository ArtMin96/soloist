//! The durable store of coordination todos and the port over it (context C6).
//!
//! Like the scratchpad port, this is deliberately small and **atomic**: every operation that depends
//! on a todo's current state — the revision-guarded [`write_doc`](TodoRepo::write_doc), the
//! read-modify-write of tags/blockers/comments, the conditional lock acquire/release — is one
//! indivisible method the adapter performs under a single guard, never a read the aggregate then acts
//! on, so two agents touching one project's todos cannot interleave to clobber an edit or double-grant
//! a lock. The revision *guard* (a doc write applies only at the expected revision) is part of this
//! contract; the disciplined-document and blocker-gate policy lives in the [`Todos`](super::Todos)
//! aggregate. The bounded context owns its own port (with a [`NoopTodoRepo`] default). A todo is
//! durable and survives an app restart (no launch-reconcile clear), but its **lock** is process-owned
//! and per-run, so [`clear_locks`](TodoRepo::clear_locks) drops every lock on launch while keeping the
//! todos.

use super::todo::{Comment, TodoDoc};
use crate::ids::{ProcessId, ProjectId, TodoId};
use crate::ports::StoreError;

/// A persisted todo: its store-assigned [`TodoId`] (durable, stable across runs), the project it
/// belongs to, the disciplined document (including its lifecycle status), its tags, the ids of the
/// todos that gate it, its comments, the process currently holding its lock (if any), and the
/// `revision` the next doc write must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTodo {
    pub id: TodoId,
    pub project: ProjectId,
    pub doc: TodoDoc,
    pub tags: Vec<String>,
    pub blockers: Vec<TodoId>,
    pub comments: Vec<Comment>,
    pub locked_by: Option<ProcessId>,
    pub revision: u64,
}

/// The outcome of a revision-guarded [`write_doc`](TodoRepo::write_doc): the write applied (the
/// stored row at its new revision), no todo exists under that id, or the expected revision no longer
/// matched. The stored row is boxed so the small `NotFound`/`Conflict` variants do not inflate the
/// enum to its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoWriteResult {
    Written(Box<StoredTodo>),
    NotFound,
    Conflict { actual: u64 },
}

/// The outcome of a comment edit ([`comment_update`](TodoRepo::comment_update) /
/// [`comment_delete`](TodoRepo::comment_delete)): the todo's updated row, no todo under that id, or
/// no comment under that id within the todo. The row is boxed so the small miss variants stay small.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentEdit {
    Edited(Box<StoredTodo>),
    NoTodo,
    NoComment,
}

/// Durable repository of coordination todos. One row per `(project, id)`, identified durably by a
/// store-assigned [`TodoId`] that is never reused. Every state-dependent method is atomic with
/// respect to the others.
pub trait TodoRepo: Send + Sync {
    /// Inserts a new todo in `project` with `doc` at revision 1, no tags, blockers, comments, or
    /// lock, returning the stored row with its assigned id.
    fn create(&self, project: ProjectId, doc: &TodoDoc) -> Result<StoredTodo, StoreError>;

    /// The todo `(project, id)`, or `None` if there is none.
    fn read(&self, project: ProjectId, id: TodoId) -> Result<Option<StoredTodo>, StoreError>;

    /// Every todo in `project`, ordered by id (creation order).
    fn list(&self, project: ProjectId) -> Result<Vec<StoredTodo>, StoreError>;

    /// Replaces the document of `(project, id)` with `doc`. When `expected` is `Some`, the write
    /// applies only if the current revision matches (else [`TodoWriteResult::Conflict`]); when
    /// `None`, it applies unconditionally (the `complete` shortcut). Either way the revision is
    /// bumped. [`TodoWriteResult::NotFound`] if no todo exists under the id. One atomic step.
    fn write_doc(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: &TodoDoc,
        expected: Option<u64>,
    ) -> Result<TodoWriteResult, StoreError>;

    /// Deletes `(project, id)`, returning whether one was removed.
    fn delete(&self, project: ProjectId, id: TodoId) -> Result<bool, StoreError>;

    /// The distinct tags used across `project`'s todos, sorted.
    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError>;

    /// Adds `tag` to `(project, id)` — atomic read-modify-write, idempotent. The updated row, or
    /// `None` if absent.
    fn add_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// Removes `tag` from `(project, id)` — atomic read-modify-write. The updated row, or `None` if
    /// absent.
    fn remove_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// Replaces the blocker set of `(project, id)` with `blockers` — atomic. The updated row, or
    /// `None` if absent. (The aggregate validates the blockers exist and exclude `id` first.)
    fn set_blockers(
        &self,
        project: ProjectId,
        id: TodoId,
        blockers: &[TodoId],
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// Adds `blocker` to `(project, id)` — atomic read-modify-write, idempotent. The updated row, or
    /// `None` if absent.
    fn add_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// Removes `blocker` from `(project, id)` — atomic read-modify-write. The updated row, or `None`
    /// if absent.
    fn remove_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// The subset of `blockers` that exist in `project` and are not yet done — the unmet gate of a
    /// todo. A blocker id that does not exist counts as met (a deleted blocker never deadlocks).
    fn unmet_blockers(
        &self,
        project: ProjectId,
        blockers: &[TodoId],
    ) -> Result<Vec<TodoId>, StoreError>;

    /// Locks `(project, id)` for `owner` if it is free or already the owner's — atomic conditional
    /// write ("signals, not ownership": a lock held by another is left intact). Returns the row with
    /// whoever now holds it, or `None` if absent.
    fn lock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// Releases the lock on `(project, id)` if held by `owner` — atomic. Returns the row, or `None`
    /// if absent. A lock held by another process is left intact (owner-close handles those).
    fn unlock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError>;

    /// Appends a comment with `body` to `(project, id)`, assigning the next per-todo comment id —
    /// atomic. Returns the updated row and the new comment id, or `None` if the todo is absent.
    fn comment_create(
        &self,
        project: ProjectId,
        id: TodoId,
        body: &str,
    ) -> Result<Option<(StoredTodo, u64)>, StoreError>;

    /// Updates comment `comment` of `(project, id)` to `body` — atomic read-modify-write.
    fn comment_update(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<CommentEdit, StoreError>;

    /// Deletes comment `comment` of `(project, id)` — atomic read-modify-write.
    fn comment_delete(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
    ) -> Result<CommentEdit, StoreError>;

    /// Releases every todo lock held by `process` — the owner-close hook. Returns how many were
    /// released.
    fn release_owner(&self, process: ProcessId) -> Result<usize, StoreError>;

    /// Clears every todo lock — launch reconciliation. A lock is process-owned and per-run ids are
    /// recycled, so no lock left by a previous run can be matched to a live process; the todos
    /// themselves are kept. Returns how many locks were cleared.
    fn clear_locks(&self) -> Result<usize, StoreError>;
}

/// A [`TodoRepo`] that stores nothing — the default until the durable adapter is wired, so the core
/// runs (todos simply never persist) without it. A create echoes a placeholder row back and every
/// read is empty.
#[derive(Clone, Copy, Default)]
pub struct NoopTodoRepo;

impl TodoRepo for NoopTodoRepo {
    fn create(&self, project: ProjectId, doc: &TodoDoc) -> Result<StoredTodo, StoreError> {
        Ok(StoredTodo {
            id: TodoId::from_raw(0),
            project,
            doc: doc.clone(),
            tags: Vec::new(),
            blockers: Vec::new(),
            comments: Vec::new(),
            locked_by: None,
            revision: 1,
        })
    }
    fn read(&self, _project: ProjectId, _id: TodoId) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn list(&self, _project: ProjectId) -> Result<Vec<StoredTodo>, StoreError> {
        Ok(Vec::new())
    }
    fn write_doc(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _doc: &TodoDoc,
        _expected: Option<u64>,
    ) -> Result<TodoWriteResult, StoreError> {
        Ok(TodoWriteResult::NotFound)
    }
    fn delete(&self, _project: ProjectId, _id: TodoId) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn tags(&self, _project: ProjectId) -> Result<Vec<String>, StoreError> {
        Ok(Vec::new())
    }
    fn add_tag(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn remove_tag(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn set_blockers(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _blockers: &[TodoId],
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn add_blocker(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn remove_blocker(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn unmet_blockers(
        &self,
        _project: ProjectId,
        _blockers: &[TodoId],
    ) -> Result<Vec<TodoId>, StoreError> {
        Ok(Vec::new())
    }
    fn lock(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn unlock(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
    fn comment_create(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _body: &str,
    ) -> Result<Option<(StoredTodo, u64)>, StoreError> {
        Ok(None)
    }
    fn comment_update(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _comment: u64,
        _body: &str,
    ) -> Result<CommentEdit, StoreError> {
        Ok(CommentEdit::NoTodo)
    }
    fn comment_delete(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _comment: u64,
    ) -> Result<CommentEdit, StoreError> {
        Ok(CommentEdit::NoTodo)
    }
    fn release_owner(&self, _process: ProcessId) -> Result<usize, StoreError> {
        Ok(0)
    }
    fn clear_locks(&self) -> Result<usize, StoreError> {
        Ok(0)
    }
}
