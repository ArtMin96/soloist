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
//! completed while a blocker is still open — the gate), **comments**, and a process-owned **lock**.
//! The durable [`TodoRepo`](super::TodoRepo) performs each state-dependent step atomically.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::todo_repo::{CommentEdit, StoredTodo, TodoRepo, TodoWriteResult};
use crate::ids::{ProcessId, ProjectId, TodoId};
use crate::ports::StoreError;

/// The most text a todo's document may carry, summed across its fields, in bytes. A todo is a
/// work-item specification, not a document store; this bounds the persisted row so a runaway
/// caller cannot grow the table without limit. Tags, blockers, and comments are separate columns,
/// each mutated by its own operation, so they are not counted here.
pub const MAX_TODO_DOC_BYTES: usize = 64 * 1024;

/// A todo's lifecycle status — the label the owning agent declares, a closed set so it is never a
/// free-form string. Distinct from the *blocker gate*: an agent may mark a todo `Blocked` to
/// communicate, but what mechanically prevents completion is its unmet [`blockers`](TodoView::blockers),
/// not this label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    /// Not yet started.
    Open,
    /// Deliberately parked by the owner (a declared label, independent of the blocker gate).
    Blocked,
    /// Being worked on.
    InProgress,
    /// Finished. Reached only when every blocker is met (the gate).
    Done,
}

/// The small document every todo carries — the revision-guarded specification of the work: a title,
/// a free-form Markdown body, and the lifecycle status. The aggregate validates it on write
/// ([`validate`](TodoDoc::validate)). Tags, blockers, comments, and the lock are **not** part of the
/// document — they are live state mutated by their own operations, so a tag or comment change never
/// collides with a concurrent specification edit (mirroring the scratchpad split of body vs
/// tags/archived).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoDoc {
    /// A short imperative title — what this todo is.
    pub title: String,
    /// The free-form Markdown body: what needs doing and any detail a worker needs to act on it.
    pub body: String,
    /// The lifecycle status the owner declares.
    pub status: TodoStatus,
}

impl TodoDoc {
    /// Checks the write is well-formed: the title is not blank and the body stays within the size
    /// cap. The body may be blank — a blank document is valid; only the title and the size ceiling
    /// are enforced. Returns a single message naming every problem at once, or `Ok(())` when it is
    /// well-formed. The status is a closed enum, so it needs no validation.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems: Vec<String> = Vec::new();
        if self.title.trim().is_empty() {
            problems.push("title must not be blank".to_owned());
        }
        if self.content_bytes() > MAX_TODO_DOC_BYTES {
            problems.push(format!(
                "the document exceeds the {} KiB cap",
                MAX_TODO_DOC_BYTES / 1024
            ));
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }

    /// The total bytes of the document's text — what a size cap bounds.
    fn content_bytes(&self) -> usize {
        self.title.len() + self.body.len()
    }
}

/// Who wrote a comment, captured when it was created so the to-do board can name the author. A
/// bound process records its per-run [`ProcessId`] — informational only, since process ids are
/// recycled across runs — alongside the `label` resolved at creation, which is durable and is what
/// the board shows. An external caller records only its label. The core stamps this from the
/// caller's resolved identity, so an author can never be forged; an unbound caller leaves a comment
/// unattributed ([`Comment::author`] is `None`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommentAuthor {
    /// A Soloist-supervised process: the live id it was bound to and the durable label shown.
    Process { id: ProcessId, label: String },
    /// An external (non-supervised) caller, identified by the label it registered under.
    External { label: String },
}

/// A comment on a todo: a per-todo sequential `id` (so an update or delete can name it), its body,
/// and its author when the creating caller was attributable ([`None`] for an unbound caller).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    /// Who created the comment, stamped by the core from the caller's identity (never caller-set).
    /// `None` when the caller was unbound. Defaulted so comments written before authorship existed
    /// read back unattributed.
    #[serde(default)]
    pub author: Option<CommentAuthor>,
}

/// A todo as a caller reads it: its durable id, its document, its tags, the ids of the
/// todos that gate it and the subset still unmet, whether it is blocked, its comments, the process
/// holding its lock (if any), and the revision to guard the next document write with.
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
    pub revision: u64,
}

/// The outcome of a comment edit through the aggregate: the todo's updated view, no todo under that
/// id, or no comment under that id within the todo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentOutcome {
    Edited(Box<TodoView>),
    NoTodo,
    NoComment,
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
    repo: Arc<dyn TodoRepo>,
}

impl Todos {
    /// Builds the aggregate over its durable store.
    pub fn new(repo: Arc<dyn TodoRepo>) -> Self {
        Self { repo }
    }

    /// Creates a todo in `project` from `doc` (validated first), returning it at
    /// revision 1. A malformed document is [`TodoError::Invalid`] and creates nothing.
    pub fn create(&self, project: ProjectId, doc: TodoDoc) -> Result<TodoView, TodoError> {
        doc.validate().map_err(TodoError::Invalid)?;
        let stored = self.repo.create(project, &doc)?;
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

    /// Replaces the document of todo `id` in `project` with `doc`, **revision-guarded** by `expected`.
    /// Validates the document, refuses a stale write ([`TodoError::Conflict`]), and — when the new
    /// status is [`Done`](TodoStatus::Done) — enforces the blocker gate ([`TodoError::Blocked`]).
    pub fn update(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: TodoDoc,
        expected: u64,
    ) -> Result<TodoView, TodoError> {
        doc.validate().map_err(TodoError::Invalid)?;
        if doc.status == TodoStatus::Done {
            self.guard_blockers(project, id)?;
        }
        self.apply_doc(project, id, &doc, Some(expected))
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
        self.apply_doc(project, id, &stored.doc, None)
    }

    /// Deletes todo `id` in `project`, returning whether one was removed.
    pub fn delete(&self, project: ProjectId, id: TodoId) -> Result<bool, StoreError> {
        self.repo.delete(project, id)
    }

    /// Moves todo `id` from `from` to `to`, clearing its blockers and lock (both reference the
    /// source project) and keeping its document, tags, comments, revision, and id. Returns the
    /// moved todo's view, or `None` if `from` has no such todo.
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

    /// Replaces the blockers of todo `id` in `project` with `blockers`, after checking none is `id`
    /// itself ([`TodoError::SelfBlocker`]) and each exists in the project ([`TodoError::UnknownBlocker`]).
    pub fn set_blockers(
        &self,
        project: ProjectId,
        id: TodoId,
        blockers: Vec<TodoId>,
    ) -> Result<TodoView, TodoError> {
        for &blocker in &blockers {
            self.check_blocker(project, id, blocker)?;
        }
        self.require(self.repo.set_blockers(project, id, &blockers)?)
    }

    /// Adds `blocker` to todo `id` in `project` (idempotent), after the same self/existence checks.
    pub fn add_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, TodoError> {
        self.check_blocker(project, id, blocker)?;
        self.require(self.repo.add_blocker(project, id, blocker)?)
    }

    /// Removes `blocker` from todo `id` in `project`, returning the updated todo, or `None` if there
    /// is none. Removing a blocker that is not present is a no-op.
    pub fn remove_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<TodoView>, StoreError> {
        self.view_opt(self.repo.remove_blocker(project, id, blocker)?)
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

    /// Adds a comment to todo `id` in `project`, attributed to `author` (stamped by the façade from
    /// the caller's identity), returning the updated todo and the new comment's id, or `None` if
    /// there is none.
    pub fn comment_create(
        &self,
        project: ProjectId,
        id: TodoId,
        body: &str,
        author: Option<CommentAuthor>,
    ) -> Result<Option<(TodoView, u64)>, StoreError> {
        match self.repo.comment_create(project, id, body, author)? {
            Some((stored, comment)) => Ok(Some((self.view(stored)?, comment))),
            None => Ok(None),
        }
    }

    /// Updates comment `comment` of todo `id` in `project` to `body`.
    pub fn comment_update(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<CommentOutcome, StoreError> {
        self.comment_outcome(self.repo.comment_update(project, id, comment, body)?)
    }

    /// Deletes comment `comment` of todo `id` in `project`.
    pub fn comment_delete(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
    ) -> Result<CommentOutcome, StoreError> {
        self.comment_outcome(self.repo.comment_delete(project, id, comment)?)
    }

    /// The comments on todo `id` in `project`, or `None` if there is none.
    pub fn comment_list(
        &self,
        project: ProjectId,
        id: TodoId,
    ) -> Result<Option<Vec<Comment>>, StoreError> {
        Ok(self.repo.read(project, id)?.map(|stored| stored.comments))
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

    /// Rejects a blocker that is the todo itself or that does not exist in the project.
    fn check_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<(), TodoError> {
        if blocker == id {
            return Err(TodoError::SelfBlocker);
        }
        if self.repo.read(project, blocker)?.is_none() {
            return Err(TodoError::UnknownBlocker);
        }
        Ok(())
    }

    /// Refuses completion while todo `id` has unmet blockers, naming them.
    fn guard_blockers(&self, project: ProjectId, id: TodoId) -> Result<(), TodoError> {
        let Some(stored) = self.repo.read(project, id)? else {
            return Err(TodoError::NotFound);
        };
        let unmet = self.repo.unmet_blockers(project, &stored.blockers)?;
        if unmet.is_empty() {
            Ok(())
        } else {
            Err(TodoError::Blocked { by: unmet })
        }
    }

    /// Writes `doc` to todo `id` and projects the result, translating the store outcome to the
    /// aggregate's errors.
    fn apply_doc(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: &TodoDoc,
        expected: Option<u64>,
    ) -> Result<TodoView, TodoError> {
        match self.repo.write_doc(project, id, doc, expected)? {
            TodoWriteResult::Written(stored) => Ok(self.view(*stored)?),
            TodoWriteResult::NotFound => Err(TodoError::NotFound),
            TodoWriteResult::Conflict { actual } => Err(TodoError::Conflict {
                expected,
                actual: Some(actual),
            }),
        }
    }

    /// Builds a [`TodoView`] from a stored row, resolving its unmet blockers.
    fn view(&self, stored: StoredTodo) -> Result<TodoView, StoreError> {
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
            revision: stored.revision,
        })
    }

    /// Projects an optional stored row to an optional view.
    fn view_opt(&self, stored: Option<StoredTodo>) -> Result<Option<TodoView>, StoreError> {
        stored.map(|stored| self.view(stored)).transpose()
    }

    /// Projects a stored row that must exist (the aggregate already checked) to a view, mapping a
    /// concurrent disappearance to [`TodoError::NotFound`].
    fn require(&self, stored: Option<StoredTodo>) -> Result<TodoView, TodoError> {
        match stored {
            Some(stored) => Ok(self.view(stored)?),
            None => Err(TodoError::NotFound),
        }
    }

    /// Translates a repo [`CommentEdit`] to the aggregate's [`CommentOutcome`], building the view.
    fn comment_outcome(&self, edit: CommentEdit) -> Result<CommentOutcome, StoreError> {
        Ok(match edit {
            CommentEdit::Edited(stored) => CommentOutcome::Edited(Box::new(self.view(*stored)?)),
            CommentEdit::NoTodo => CommentOutcome::NoTodo,
            CommentEdit::NoComment => CommentOutcome::NoComment,
        })
    }
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
