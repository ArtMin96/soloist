//! The todo aggregate (context C6): durable, project-scoped work items agents hand off and
//! coordinate around.
//!
//! Like a scratchpad and unlike a lease or timer, a todo is **durable** — it survives an app restart,
//! so launch reconciliation never clears the todo (only its process-owned *lock*, [`§
//! TodoLockReleaser`](super::TodoLockReleaser)). A todo carries a small document ([`TodoDoc`]): a
//! title, a free-form Markdown body, and a lifecycle status. The body is unconstrained (bounded only
//! by a size cap), so a caller shapes it freely — a template seeds it, but the schema is not
//! enforced; the `status` stays a closed enum (workflow state, not prose). The document is
//! **revision-guarded** (optimistic concurrency): an [`update`](Todos::update) carries the revision it
//! expects, and a stale one is refused rather than clobbering a newer edit. Around the document sit
//! live columns the dedicated operations mutate atomically — **tags**, **blockers** (a todo cannot be
//! completed while a blocker is still open — the gate, in [`todo_blocker`](super::todo_blocker)),
//! **comments** (in [`todo_comment`](super::todo_comment)), a process-owned **lock**, and an
//! optional **scratchpad** association naming the document the todo was derived from. That
//! association is live state, not part of the document, so it survives a specification edit and is
//! never validated: having none is a permanently valid state, not a gap to fill.
//! The durable [`TodoRepo`](super::TodoRepo) performs each state-dependent step atomically.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::scratchpad::ScratchpadRef;
use super::scratchpad_link::ScratchpadLink;
use super::todo_comment::Comment;
use super::todo_doc::{TodoDoc, TodoStatus};
use super::todo_repo::{StoredTodo, TodoRepo, TodoWriteResult};
use crate::ids::{ProcessId, ProjectId, ScratchpadId, TodoId};
use crate::ports::StoreError;

/// A todo as a caller reads it: its durable id, its document, its tags, the ids of the
/// todos that gate it and the subset still unmet, whether it is blocked, its comments, the process
/// holding its lock (if any), the scratchpad it derives from (if any), and the revision to guard
/// the next document write with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoView {
    pub id: TodoId,
    pub doc: TodoDoc,
    pub tags: Vec<String>,
    pub blockers: Vec<TodoId>,
    /// The blockers that still exist and are not done — empty once the todo is free to complete.
    pub blocked_by: Vec<TodoId>,
    /// Whether the todo is currently gated (`blocked_by` non-empty), surfaced for convenience.
    pub blocked: bool,
    pub comments: Vec<Comment>,
    pub locked_by: Option<ProcessId>,
    /// The scratchpad this todo was derived from, or `None` — the permanently valid default. A todo
    /// never requires one; it is linked only when it came out of that document.
    pub scratchpad: Option<ScratchpadRef>,
    pub revision: u64,
}

/// A todo in a listing: enough to scan and pick one without fetching every document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoSummary {
    pub id: TodoId,
    pub title: String,
    pub status: TodoStatus,
    pub tags: Vec<String>,
    pub blocked: bool,
    pub locked_by: Option<ProcessId>,
    /// The scratchpad this todo was derived from, or `None` (see [`TodoView::scratchpad`]).
    pub scratchpad: Option<ScratchpadRef>,
    pub revision: u64,
}

/// Why a todo create, update, complete, or blocker change was refused — each the caller's to fix.
#[derive(Debug, thiserror::Error)]
pub enum TodoError {
    /// The document failed validation (a blank title or an over-cap body); the message names every problem.
    #[error("todo is not well-formed: {0}")]
    Invalid(String),
    /// The action named a todo that does not exist in the project.
    #[error("no todo under that id")]
    NotFound,
    /// An update expected a different revision than the one on record — a concurrent edit landed
    /// first, so the write was refused. `expected` is what the caller passed; `actual` the row's.
    #[error("todo revision conflict (expected {expected:?}, found {actual:?})")]
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// Completion was refused because the todo still has unmet blockers (the gate). `by` lists them.
    #[error("todo is blocked by {by:?}")]
    Blocked { by: Vec<TodoId> },
    /// A blocker referenced a todo that does not exist in the project.
    #[error("no todo under that id to block on")]
    UnknownBlocker,
    /// A todo cannot block itself.
    #[error("a todo cannot block itself")]
    SelfBlocker,
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The todo aggregate over the durable [`TodoRepo`]. The repo persists and makes each state-dependent
/// step atomic; this aggregate owns the document validation, the revision-guard policy,
/// and the blocker gate. Cheap to clone-share via the `Arc` it holds.
pub struct Todos {
    pub(super) repo: Arc<dyn TodoRepo>,
}

impl Todos {
    /// Builds the aggregate over its durable store.
    pub fn new(repo: Arc<dyn TodoRepo>) -> Self {
        Self { repo }
    }

    /// Creates a todo in `project` from `doc` (validated first), optionally associated with the
    /// scratchpad it derives from, returning it at revision 1. A malformed document is
    /// [`TodoError::Invalid`] and creates nothing. `scratchpad` is `None` for a todo that came from
    /// nowhere in particular — the default, and never a validation failure.
    pub fn create(
        &self,
        project: ProjectId,
        doc: TodoDoc,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<TodoView, TodoError> {
        doc.validate().map_err(TodoError::Invalid)?;
        let stored = self.repo.create(project, &doc, scratchpad)?;
        Ok(self.view(stored)?)
    }

    /// The todo `id` in `project`, or `None` if there is none.
    pub fn get(&self, project: ProjectId, id: TodoId) -> Result<Option<TodoView>, StoreError> {
        self.repo
            .read(project, id)?
            .map(|stored| self.view(stored))
            .transpose()
    }

    /// Every todo in `project` as a one-line summary, in creation order.
    pub fn list(&self, project: ProjectId) -> Result<Vec<TodoSummary>, StoreError> {
        self.repo
            .list(project)?
            .into_iter()
            .map(|stored| self.summary(stored))
            .collect()
    }

    /// Every todo in `project` as a full [`TodoView`] (blockers, comments, lock owner, the derived
    /// `blocked` flag), in creation order — the read the orchestration to-do board projects. Reuses
    /// the same store read as [`list`](Self::list) and the same per-row [`view`](Self::view)
    /// mapping, so it is no more costly than the summary list and stays a single source of truth.
    pub fn views(&self, project: ProjectId) -> Result<Vec<TodoView>, StoreError> {
        self.repo
            .list(project)?
            .into_iter()
            .map(|stored| self.view(stored))
            .collect()
    }

    /// Replaces the document of todo `id` in `project` with `doc`, **revision-guarded** by `expected`,
    /// and applies `scratchpad` to its association. Validates the document, refuses a stale write
    /// ([`TodoError::Conflict`]), and — when the new status is [`Done`](TodoStatus::Done) — enforces
    /// the blocker gate ([`TodoError::Blocked`]). The document is replaced wholesale; the
    /// association is not part of it, so [`ScratchpadLink::Unchanged`] leaves it standing.
    pub fn update(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: TodoDoc,
        scratchpad: ScratchpadLink<ScratchpadId>,
        expected: u64,
    ) -> Result<TodoView, TodoError> {
        doc.validate().map_err(TodoError::Invalid)?;
        if doc.status == TodoStatus::Done {
            self.guard_blockers(project, id)?;
        }
        self.apply_doc(project, id, &doc, scratchpad, Some(expected))
    }

    /// Marks todo `id` in `project` done — the convenience over [`update`](Self::update) that needs no
    /// revision. Enforces the same blocker gate: refused with [`TodoError::Blocked`] while any blocker
    /// is unmet, so a todo stays gated until its blockers complete.
    pub fn complete(&self, project: ProjectId, id: TodoId) -> Result<TodoView, TodoError> {
        let Some(mut stored) = self.repo.read(project, id)? else {
            return Err(TodoError::NotFound);
        };
        self.guard_blockers(project, id)?;
        stored.doc.status = TodoStatus::Done;
        self.apply_doc(project, id, &stored.doc, ScratchpadLink::Unchanged, None)
    }

    /// Deletes todo `id` in `project`, returning whether one was removed.
    pub fn delete(&self, project: ProjectId, id: TodoId) -> Result<bool, StoreError> {
        self.repo.delete(project, id)
    }

    /// Moves todo `id` from `from` to `to`, clearing its blockers, lock, and scratchpad association
    /// (all three reference the source project) and keeping its document, tags, comments, revision,
    /// and id. Returns the moved todo's view, or `None` if `from` has no such todo.
    pub fn transfer(
        &self,
        from: ProjectId,
        to: ProjectId,
        id: TodoId,
    ) -> Result<Option<TodoView>, StoreError> {
        self.view_opt(self.repo.transfer(from, to, id)?)
    }

    /// The distinct tags used across `project`'s todos, sorted.
    pub fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        self.repo.tags(project)
    }

    /// Adds `tag` to todo `id` in `project` (idempotent), returning the updated todo, or `None` if
    /// there is none.
    pub fn add_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<TodoView>, StoreError> {
        self.view_opt(self.repo.add_tag(project, id, tag)?)
    }

    /// Removes `tag` from todo `id` in `project`, returning the updated todo, or `None` if there is
    /// none.
    pub fn remove_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<TodoView>, StoreError> {
        self.view_opt(self.repo.remove_tag(project, id, tag)?)
    }

    /// Locks todo `id` in `project` for `owner` — "signals, not ownership": if another process holds
    /// the lock it is left intact and the returned view reports that holder, so the caller compares
    /// `locked_by` to its own process to know whether it won. `None` if the todo does not exist.
    pub fn lock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<TodoView>, StoreError> {
        self.view_opt(self.repo.lock(project, id, owner)?)
    }

    /// Releases the lock on todo `id` in `project` if held by `owner`, returning the updated todo, or
    /// `None` if there is none. A lock another process holds is left intact.
    pub fn unlock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<TodoView>, StoreError> {
        self.view_opt(self.repo.unlock(project, id, owner)?)
    }

    /// Releases every todo lock held by `process` — the owner-close hook (see
    /// [`TodoLockReleaser`](super::TodoLockReleaser)). Returns how many were released.
    pub fn release_owner(&self, process: ProcessId) -> Result<usize, StoreError> {
        self.repo.release_owner(process)
    }

    /// Clears every todo lock — launch reconciliation. The todos persist; only their process-owned
    /// locks, whose owners are gone and whose per-run ids are recycled, are dropped.
    pub fn reconcile(&self) -> Result<usize, StoreError> {
        self.repo.clear_locks()
    }

    /// Writes `doc` and the stated `scratchpad` link to todo `id` and projects the result,
    /// translating the store outcome to the aggregate's errors.
    fn apply_doc(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: &TodoDoc,
        scratchpad: ScratchpadLink<ScratchpadId>,
        expected: Option<u64>,
    ) -> Result<TodoView, TodoError> {
        match self
            .repo
            .write_doc(project, id, doc, scratchpad, expected)?
        {
            TodoWriteResult::Written(stored) => Ok(self.view(*stored)?),
            TodoWriteResult::NotFound => Err(TodoError::NotFound),
            TodoWriteResult::Conflict { actual } => Err(TodoError::Conflict {
                expected,
                actual: Some(actual),
            }),
        }
    }

    /// Builds a [`TodoView`] from a stored row, resolving its unmet blockers.
    pub(super) fn view(&self, stored: StoredTodo) -> Result<TodoView, StoreError> {
        let blocked_by = self.repo.unmet_blockers(stored.project, &stored.blockers)?;
        Ok(TodoView {
            id: stored.id,
            doc: stored.doc,
            tags: stored.tags,
            blockers: stored.blockers,
            blocked: !blocked_by.is_empty(),
            blocked_by,
            comments: stored.comments,
            locked_by: stored.locked_by,
            scratchpad: stored.scratchpad,
            revision: stored.revision,
        })
    }

    /// Builds a [`TodoSummary`] from a stored row, resolving whether it is blocked.
    fn summary(&self, stored: StoredTodo) -> Result<TodoSummary, StoreError> {
        let blocked = !self
            .repo
            .unmet_blockers(stored.project, &stored.blockers)?
            .is_empty();
        Ok(TodoSummary {
            id: stored.id,
            title: stored.doc.title,
            status: stored.doc.status,
            tags: stored.tags,
            blocked,
            locked_by: stored.locked_by,
            scratchpad: stored.scratchpad,
            revision: stored.revision,
        })
    }

    /// Projects an optional stored row to an optional view.
    pub(super) fn view_opt(
        &self,
        stored: Option<StoredTodo>,
    ) -> Result<Option<TodoView>, StoreError> {
        stored.map(|stored| self.view(stored)).transpose()
    }
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
