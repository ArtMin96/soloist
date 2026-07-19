//! Session-scoped todo actions (context C8 → C6): the durable shared-work-item surface a remote
//! caller (MCP today) drives within its effective project.
//!
//! Todos are project-scoped durable content, so most methods resolve only the session's **effective
//! project** (reusing [`coordination_scope`](Facade::coordination_scope), shared with the lease,
//! timer, and scratchpad surface) — an external single-project caller can use them without binding a
//! process. The two **lock** actions are the exception: a todo lock is process-owned, so they also
//! resolve the **bound process** (reusing [`coordination_owner`](Facade::coordination_owner)) — the
//! owner the supervisor auto-releases the lock for on close. A remote caller addresses a scratchpad
//! by its `name` handle, so an association it states is resolved to the durable id **here, within
//! the session's scope**: an unknown name is refused before anything is written, and a name can
//! never reach across into another project's documents.

use super::scoped::ScopedFacade;
use super::todo::{map_comment, TodoCreation};
use crate::coordination::{Comment, CommentAuthor, ScratchpadLink, TodoDoc, TodoSummary, TodoView};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::identity::Origin;
use crate::ids::{ProjectId, ScratchpadId, TodoId};

impl ScopedFacade<'_> {
    /// Creates a todo from `doc` in the session's effective project, optionally associated with the
    /// scratchpad `scratchpad` names — the document it was derived from. A malformed document is
    /// [`CoordinationError::InvalidTodo`] and an unresolvable name
    /// [`CoordinationError::UnknownScratchpad`], neither of which creates anything; an empty body is
    /// seeded from the default todo template, and the returned [`TodoCreation`] names it.
    pub fn todo_create(
        &self,
        doc: TodoDoc,
        scratchpad: Option<String>,
    ) -> Result<TodoCreation, CoordinationError> {
        let project = self.coordination_scope()?;
        let scratchpad = self.resolve_scratchpad(project, scratchpad)?;
        self.inner.create_todo(project, doc, scratchpad)
    }

    /// Every todo in the session's effective project, as one-line summaries.
    pub fn todo_list(&self) -> Result<Vec<TodoSummary>, CoordinationError> {
        let project = self.coordination_scope()?;
        Ok(self.inner.todos.list(project)?)
    }

    /// The todo `id` in the session's effective project, or [`CoordinationError::UnknownTodo`].
    pub fn todo_get(&self, id: TodoId) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner
            .todos
            .get(project, id)?
            .ok_or(CoordinationError::UnknownTodo)
    }

    /// Replaces the document of todo `id` with `doc` in the session's effective project,
    /// **revision-guarded** by `expected`, and applies `scratchpad` to its association; setting
    /// status to done is gated on the todo's blockers. A [`ScratchpadLink::Unchanged`] leaves the
    /// association standing, and an unresolvable name is [`CoordinationError::UnknownScratchpad`]
    /// with nothing written.
    pub fn todo_update(
        &self,
        id: TodoId,
        doc: TodoDoc,
        scratchpad: ScratchpadLink<String>,
        expected: u64,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        let scratchpad = scratchpad.try_map(|name| self.scratchpad_id(project, &name))?;
        self.inner
            .todo_update_in(project, id, doc, scratchpad, expected)
    }

    /// Marks todo `id` done in the session's effective project — refused with
    /// [`CoordinationError::TodoBlocked`] while it has unmet blockers (the gate).
    pub fn todo_complete(&self, id: TodoId) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.todo_complete_in(project, id)
    }

    /// Deletes todo `id` in the session's effective project, returning whether one was removed.
    pub fn todo_delete(&self, id: TodoId) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope()?;
        let removed = self.inner.todos.delete(project, id)?;
        if removed {
            self.inner
                .bus
                .publish(DomainEvent::TodoChanged { project, id });
        }
        Ok(removed)
    }

    /// The distinct tags used across the session's effective project's todos, sorted.
    pub fn todo_tags_list(&self) -> Result<Vec<String>, CoordinationError> {
        let project = self.coordination_scope()?;
        Ok(self.inner.todos.tags(project)?)
    }

    /// Adds `tag` to todo `id` in the session's effective project, returning the updated todo, or
    /// [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_add_tag(&self, id: TodoId, tag: &str) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.emit_todo(
            project,
            self.inner
                .todos
                .add_tag(project, id, tag)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Removes `tag` from todo `id` in the session's effective project, returning the updated todo,
    /// or [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_remove_tag(&self, id: TodoId, tag: &str) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.emit_todo(
            project,
            self.inner
                .todos
                .remove_tag(project, id, tag)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Replaces the blockers of todo `id` in the session's effective project, after validating each
    /// exists and is not the todo itself.
    pub fn todo_set_blockers(
        &self,
        id: TodoId,
        blockers: Vec<TodoId>,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.todo_set_blockers_in(project, id, blockers)
    }

    /// Adds `blocker` to todo `id` in the session's effective project, after the same checks.
    pub fn todo_add_blocker(
        &self,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.todo_add_blocker_in(project, id, blocker)
    }

    /// Removes `blocker` from todo `id` in the session's effective project, returning the updated
    /// todo, or [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_remove_blocker(
        &self,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.todo_remove_blocker_in(project, id, blocker)
    }

    /// Moves todo `id` into project `to` for a scoped session (context C8 → C6). Authorized
    /// only when the caller is authenticated to **both** its own effective project (the source) and
    /// `to` (the target, via [`authentic_scope`](Facade::authentic_scope)); otherwise
    /// [`CoordinationError::ForeignProject`]. Because an MCP session authenticates to a single
    /// project, a genuine cross-project transfer is refused here — the reachable path is the local
    /// [`todo_transfer_in`](Facade::todo_transfer_in). Preserves comments/completion, clears
    /// blockers, lock, and the scratchpad association.
    pub fn todo_transfer(&self, to: ProjectId, id: TodoId) -> Result<TodoView, CoordinationError> {
        let from = self.coordination_scope()?;
        if !self.authentic_scope(to) {
            return Err(CoordinationError::ForeignProject);
        }
        self.inner.todo_transfer_in(from, to, id)
    }

    /// Locks todo `id` in the session's effective project for the caller's bound process —
    /// "signals, not ownership": the returned todo's `locked_by` reports the holder, so the caller
    /// checks whether it won. Needs a bound process (the owner the lock auto-releases for on close).
    pub fn todo_lock(&self, id: TodoId) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        let owner = self.coordination_owner()?;
        self.inner.emit_todo(
            project,
            self.inner
                .todos
                .lock(project, id, owner)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Releases the lock on todo `id` in the session's effective project if held by the caller's
    /// bound process, returning the updated todo, or [`CoordinationError::UnknownTodo`] if there is
    /// none.
    pub fn todo_unlock(&self, id: TodoId) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        let owner = self.coordination_owner()?;
        self.inner.emit_todo(
            project,
            self.inner
                .todos
                .unlock(project, id, owner)?
                .ok_or(CoordinationError::UnknownTodo),
        )
    }

    /// Adds a comment to todo `id` in the session's effective project, returning the updated todo and
    /// the new comment's id, or [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_comment_create(
        &self,
        id: TodoId,
        body: &str,
    ) -> Result<(TodoView, u64), CoordinationError> {
        let project = self.coordination_scope()?;
        let author = self.comment_author();
        let created = self
            .inner
            .todos
            .comment_create(project, id, body, author)?
            .ok_or(CoordinationError::UnknownTodo)?;
        self.inner
            .bus
            .publish(DomainEvent::TodoChanged { project, id });
        Ok(created)
    }

    /// Updates comment `comment` of todo `id` in the session's effective project.
    pub fn todo_comment_update(
        &self,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.emit_todo(
            project,
            map_comment(
                self.inner
                    .todos
                    .comment_update(project, id, comment, body)?,
            ),
        )
    }

    /// Deletes comment `comment` of todo `id` in the session's effective project.
    pub fn todo_comment_delete(
        &self,
        id: TodoId,
        comment: u64,
    ) -> Result<TodoView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.emit_todo(
            project,
            map_comment(self.inner.todos.comment_delete(project, id, comment)?),
        )
    }

    /// The comments on todo `id` in the session's effective project, or
    /// [`CoordinationError::UnknownTodo`] if there is none.
    pub fn todo_comment_list(&self, id: TodoId) -> Result<Vec<Comment>, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner
            .todos
            .comment_list(project, id)?
            .ok_or(CoordinationError::UnknownTodo)
    }

    /// The author to stamp on a new comment, resolved in the core from the caller's identity: a
    /// bound process (its id plus the label resolved now and kept durably), an external caller (its
    /// label), or `None` for an unbound caller. The caller never supplies this, so the author of a
    /// comment cannot be forged.
    fn comment_author(&self) -> Option<CommentAuthor> {
        match self.inner.identity.origin(self.session) {
            Origin::Process(id) => Some(CommentAuthor::Process {
                id,
                label: self
                    .inner
                    .process_view(id)
                    .map(|view| view.label)
                    .unwrap_or_else(|| format!("process {}", id.get())),
            }),
            Origin::External(label) => Some(CommentAuthor::External { label }),
            Origin::Unbound => None,
        }
    }

    /// The scratchpad a create names, resolved to its durable id within `project` — `None` when the
    /// caller named none, which is the ordinary unlinked todo.
    fn resolve_scratchpad(
        &self,
        project: ProjectId,
        name: Option<String>,
    ) -> Result<Option<ScratchpadId>, CoordinationError> {
        name.map(|name| self.scratchpad_id(project, &name))
            .transpose()
    }

    /// The durable id of the scratchpad `name` in `project`, or
    /// [`CoordinationError::UnknownScratchpad`]. Resolving inside the session's own scope is what
    /// keeps a name from reaching another project's documents.
    fn scratchpad_id(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<ScratchpadId, CoordinationError> {
        Ok(self.inner.scratchpad_read_in(project, name)?.id)
    }
}
