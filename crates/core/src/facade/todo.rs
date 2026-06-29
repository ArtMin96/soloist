//! Session-scoped todo actions (context C8 → C6): the durable shared-work-item surface a remote
//! caller (MCP today) drives within its effective project.
//!
//! Todos are project-scoped durable content, so most methods resolve only the session's **effective
//! project** (reusing [`coordination_scope`](Facade::coordination_scope), shared with the lease,
//! timer, and scratchpad surface) — an external single-project caller can use them without binding a
//! process. The two **lock** actions are the exception: a todo lock is process-owned, so they also
//! resolve the **bound process** (reusing [`coordination_owner`](Facade::coordination_owner)) — the
//! owner the supervisor auto-releases the lock for on close. Scope and ownership are resolved here, in
//! the core, so every remote surface inherits the identical rules; this surface maps the aggregate's
//! typed outcomes to the shared [`CoordinationError`].

use super::Facade;
use crate::coordination::{
    Comment, CommentAuthor, CommentOutcome, TodoDoc, TodoError, TodoSummary, TodoView,
};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::identity::Origin;
use crate::ids::{ProjectId, SessionId, TodoId};
use crate::ports::StoreError;

impl Facade {
    /// Creates a todo from the disciplined `doc` in the session's effective project. A malformed
    /// document is [`CoordinationError::InvalidTodo`].
    pub fn todo_create(
        &self,
        session: SessionId,
        doc: TodoDoc,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todo_create_in(project, doc)
    }

    /// Every todo in the session's effective project, as one-line summaries.
    pub fn todo_list(&self, session: SessionId) -> Result<Vec<TodoSummary>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        Ok(self.todos.list(project)?)
    }

    /// The todo `id` in the session's effective project, or [`CoordinationError::UnknownTodo`].
    pub fn todo_get(&self, session: SessionId, id: TodoId) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todos
            .get(project, id)?
            .ok_or(CoordinationError::UnknownTodo)
    }

    /// Replaces the document of todo `id` with `doc` in the session's effective project,
    /// **revision-guarded** by `expected`; setting status to done is gated on the todo's blockers.
    pub fn todo_update(
        &self,
        session: SessionId,
        id: TodoId,
        doc: TodoDoc,
        expected: u64,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todo_update_in(project, id, doc, expected)
    }

    /// Marks todo `id` done in the session's effective project — refused with
    /// [`CoordinationError::TodoBlocked`] while it has unmet blockers (the gate).
    pub fn todo_complete(
        &self,
        session: SessionId,
        id: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todo_complete_in(project, id)
    }

    /// Deletes todo `id` in the session's effective project, returning whether one was removed.
    pub fn todo_delete(&self, session: SessionId, id: TodoId) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let removed = self.todos.delete(project, id)?;
        if removed {
            self.bus.publish(DomainEvent::TodoChanged { project, id });
        }
        Ok(removed)
    }

    /// The distinct tags used across the session's effective project's todos, sorted.
    pub fn todo_tags_list(&self, session: SessionId) -> Result<Vec<String>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        Ok(self.todos.tags(project)?)
    }

    /// Adds `tag` to todo `id` in the session's effective project, returning the updated todo, or
    /// [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_add_tag(
        &self,
        session: SessionId,
        id: TodoId,
        tag: &str,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_todo(
            project,
            self.todos
                .add_tag(project, id, tag)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Removes `tag` from todo `id` in the session's effective project, returning the updated todo,
    /// or [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_remove_tag(
        &self,
        session: SessionId,
        id: TodoId,
        tag: &str,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_todo(
            project,
            self.todos
                .remove_tag(project, id, tag)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Replaces the blockers of todo `id` in the session's effective project, after validating each
    /// exists and is not the todo itself.
    pub fn todo_set_blockers(
        &self,
        session: SessionId,
        id: TodoId,
        blockers: Vec<TodoId>,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todo_set_blockers_in(project, id, blockers)
    }

    /// Adds `blocker` to todo `id` in the session's effective project, after the same checks.
    pub fn todo_add_blocker(
        &self,
        session: SessionId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todo_add_blocker_in(project, id, blocker)
    }

    /// Removes `blocker` from todo `id` in the session's effective project, returning the updated
    /// todo, or [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_remove_blocker(
        &self,
        session: SessionId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todo_remove_blocker_in(project, id, blocker)
    }

    // The project-scoped write surface the local-UI to-do board drives. Each mirrors its
    // session-scoped sibling above with `project` supplied directly, trusting the caller to be
    // entitled to it — like [`orchestration_snapshot`](Self::orchestration_snapshot), so none must
    // ever take a `project` from an untrusted surface. Locking stays session/owner-scoped only: the
    // board observes a lock but never acquires one ("signals, not ownership").

    /// [`todo_create`](Self::todo_create) scoped to `project` directly (local-UI path).
    pub fn todo_create_in(
        &self,
        project: ProjectId,
        doc: TodoDoc,
    ) -> Result<TodoView, CoordinationError> {
        self.emit_todo(
            project,
            self.todos.create(project, doc).map_err(map_todo_error),
        )
    }

    /// [`todo_update`](Self::todo_update) scoped to `project` directly (local-UI path).
    pub fn todo_update_in(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: TodoDoc,
        expected: u64,
    ) -> Result<TodoView, CoordinationError> {
        self.emit_todo(
            project,
            self.todos
                .update(project, id, doc, expected)
                .map_err(map_todo_error),
        )
    }

    /// [`todo_complete`](Self::todo_complete) scoped to `project` directly (local-UI path).
    pub fn todo_complete_in(
        &self,
        project: ProjectId,
        id: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        self.emit_todo(
            project,
            self.todos.complete(project, id).map_err(map_todo_error),
        )
    }

    /// [`todo_set_blockers`](Self::todo_set_blockers) scoped to `project` directly (local-UI path).
    pub fn todo_set_blockers_in(
        &self,
        project: ProjectId,
        id: TodoId,
        blockers: Vec<TodoId>,
    ) -> Result<TodoView, CoordinationError> {
        self.emit_todo(
            project,
            self.todos
                .set_blockers(project, id, blockers)
                .map_err(map_todo_error),
        )
    }

    /// [`todo_add_blocker`](Self::todo_add_blocker) scoped to `project` directly (local-UI path).
    pub fn todo_add_blocker_in(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        self.emit_todo(
            project,
            self.todos
                .add_blocker(project, id, blocker)
                .map_err(map_todo_error),
        )
    }

    /// [`todo_remove_blocker`](Self::todo_remove_blocker) scoped to `project` directly (local-UI path).
    pub fn todo_remove_blocker_in(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        self.emit_todo(
            project,
            self.todos
                .remove_blocker(project, id, blocker)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Locks todo `id` in the session's effective project for the caller's bound process —
    /// "signals, not ownership": the returned todo's `locked_by` reports the holder, so the caller
    /// checks whether it won. Needs a bound process (the owner the lock auto-releases for on close).
    pub fn todo_lock(&self, session: SessionId, id: TodoId) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let owner = self.coordination_owner(session)?;
        self.emit_todo(
            project,
            self.todos
                .lock(project, id, owner)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Releases the lock on todo `id` in the session's effective project if held by the caller's
    /// bound process, returning the updated todo, or [`CoordinationError::UnknownTodo`] if there is
    /// none.
    pub fn todo_unlock(
        &self,
        session: SessionId,
        id: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let owner = self.coordination_owner(session)?;
        self.emit_todo(
            project,
            self.todos
                .unlock(project, id, owner)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Adds a comment to todo `id` in the session's effective project, returning the updated todo and
    /// the new comment's id, or [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_comment_create(
        &self,
        session: SessionId,
        id: TodoId,
        body: &str,
    ) -> Result<(TodoView, u64), CoordinationError> {
        let project = self.coordination_scope(session)?;
        let author = self.comment_author(session);
        let created = self
            .todos
            .comment_create(project, id, body, author)?
            .ok_or(CoordinationError::UnknownTodo)?;
        self.bus.publish(DomainEvent::TodoChanged { project, id });
        Ok(created)
    }

    /// The author to stamp on a new comment, resolved in the core from the caller's identity: a
    /// bound process (its id plus the label resolved now and kept durably), an external caller (its
    /// label), or `None` for an unbound caller. The caller never supplies this, so the author of a
    /// comment cannot be forged.
    fn comment_author(&self, session: SessionId) -> Option<CommentAuthor> {
        match self.identity.origin(session) {
            Origin::Process(id) => Some(CommentAuthor::Process {
                id,
                label: self
                    .process_view(id)
                    .map(|view| view.label)
                    .unwrap_or_else(|| format!("process {}", id.get())),
            }),
            Origin::External(label) => Some(CommentAuthor::External { label }),
            Origin::Unbound => None,
        }
    }

    /// Updates comment `comment` of todo `id` in the session's effective project.
    pub fn todo_comment_update(
        &self,
        session: SessionId,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_todo(
            project,
            map_comment(self.todos.comment_update(project, id, comment, body)?),
        )
    }

    /// Deletes comment `comment` of todo `id` in the session's effective project.
    pub fn todo_comment_delete(
        &self,
        session: SessionId,
        id: TodoId,
        comment: u64,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_todo(
            project,
            map_comment(self.todos.comment_delete(project, id, comment)?),
        )
    }

    /// The comments on todo `id` in the session's effective project, or
    /// [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_comment_list(
        &self,
        session: SessionId,
        id: TodoId,
    ) -> Result<Vec<Comment>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.todos
            .comment_list(project, id)?
            .ok_or(CoordinationError::UnknownTodo)
    }

    /// Clears every stale todo lock on launch — see [`Todos::reconcile`](crate::coordination::Todos::reconcile).
    /// Not session-scoped; the composition root calls it once at startup. The todos themselves
    /// persist; only their process-owned locks (whose per-run owners are gone) are dropped.
    pub fn reconcile_todo_locks(&self) -> Result<usize, StoreError> {
        self.todos.reconcile()
    }

    /// Publishes a [`DomainEvent::TodoChanged`] for the todo a successful mutation returned, then
    /// passes the result through unchanged — the single emission seam every todo write routes
    /// through, so a change reaches every adapter (UI, MCP, HTTP) once. A failed write emits
    /// nothing.
    fn emit_todo(
        &self,
        project: ProjectId,
        result: Result<TodoView, CoordinationError>,
    ) -> Result<TodoView, CoordinationError> {
        if let Ok(view) = &result {
            self.bus.publish(DomainEvent::TodoChanged {
                project,
                id: view.id,
            });
        }
        result
    }
}

/// Maps the todo aggregate's error to the shared coordination error, defined once here so every
/// remote surface reports the same taxonomy.
fn map_todo_error(err: TodoError) -> CoordinationError {
    match err {
        TodoError::Invalid(message) => CoordinationError::InvalidTodo(message),
        TodoError::NotFound => CoordinationError::UnknownTodo,
        TodoError::Conflict { expected, actual } => {
            CoordinationError::TodoRevisionConflict { expected, actual }
        }
        TodoError::Blocked { by } => CoordinationError::TodoBlocked { by },
        TodoError::UnknownBlocker => CoordinationError::UnknownBlocker,
        TodoError::SelfBlocker => CoordinationError::SelfBlocker,
        TodoError::Store(err) => CoordinationError::Store(err),
    }
}

/// Maps a comment-edit outcome to the updated todo or the matching not-found error.
fn map_comment(outcome: CommentOutcome) -> Result<TodoView, CoordinationError> {
    match outcome {
        CommentOutcome::Edited(view) => Ok(*view),
        CommentOutcome::NoTodo => Err(CoordinationError::UnknownTodo),
        CommentOutcome::NoComment => Err(CoordinationError::UnknownComment),
    }
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
