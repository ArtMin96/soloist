//! Local-authority todo actions (context C8 → C6): the durable shared-work-item surface the desktop
//! to-do board drives, plus the create seam every caller routes through.
//!
//! Each `*_in` method takes its `project` directly — the local user's authority, mirroring
//! [`orchestration_snapshot`](Facade::orchestration_snapshot) — so none may ever take a `project`
//! from an untrusted surface. The session-scoped mirror a remote caller (MCP) drives lives in
//! [`scoped_todo`](super::scoped_todo) and routes to these same methods, so the document rules, the
//! blocker gate, and the scratchpad association behave identically whichever surface asks. This
//! module maps the aggregate's typed outcomes to the shared [`CoordinationError`].

use super::template::Seeded;
use super::Facade;
use crate::coordination::{CommentOutcome, ScratchpadLink, TodoDoc, TodoError, TodoView};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::{ProjectId, ScratchpadId, TodoId};
use crate::ports::StoreError;
use crate::template::TemplateKind;

/// The outcome of creating a todo: the new view, plus the name of the template that seeded its body
/// when it was created with an empty body (`None` when nothing seeded).
pub struct TodoCreation {
    pub view: TodoView,
    pub seeded_from: Option<String>,
}

impl Facade {
    // Locking stays session/owner-scoped only: the board observes a lock but never acquires one
    // ("signals, not ownership").

    /// [`todo_create`](super::scoped::ScopedFacade::todo_create) scoped to `project` directly
    /// (local-UI path). The board's picker always states the association, so it passes the chosen
    /// scratchpad's durable id, or `None` for the unlinked default.
    pub fn todo_create_in(
        &self,
        project: ProjectId,
        doc: TodoDoc,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<TodoView, CoordinationError> {
        Ok(self.create_todo(project, doc, scratchpad)?.view)
    }

    /// Refuses an association naming a scratchpad `project` does not own, with
    /// [`CoordinationError::UnknownScratchpad`] and nothing written.
    ///
    /// The session-scoped surface resolves a *name* inside its own scope, so the id it arrives with
    /// is in-project by construction. The local surface states the association as a durable id
    /// straight from the board's picker, and an id names a row without naming a project — so it is
    /// checked rather than trusted. Both create and update route through here, which is what keeps
    /// an out-of-project id unusable whichever surface offers it.
    fn guard_scratchpad(
        &self,
        project: ProjectId,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<(), CoordinationError> {
        match scratchpad {
            None => Ok(()),
            Some(id) if self.scratchpads.contains(project, id)? => Ok(()),
            Some(_) => Err(CoordinationError::UnknownScratchpad),
        }
    }

    /// The one todo-create seam both the local UI and MCP route through: an empty body is seeded
    /// from the default todo template, then the todo is created — associated with `scratchpad` when
    /// one was named — and its event emitted. Returns the view plus the seeding template's name for
    /// the MCP create response; the local-UI [`todo_create_in`](Self::todo_create_in) keeps just the
    /// view.
    pub(crate) fn create_todo(
        &self,
        project: ProjectId,
        mut doc: TodoDoc,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<TodoCreation, CoordinationError> {
        self.guard_scratchpad(project, scratchpad)?;
        let Seeded { body, from } = self.seed_body(TemplateKind::Todo, doc.body)?;
        doc.body = body;
        let view = self.emit_todo(
            project,
            self.todos
                .create(project, doc, scratchpad)
                .map_err(map_todo_error),
        )?;
        Ok(TodoCreation {
            view,
            seeded_from: from,
        })
    }

    /// [`todo_update`](super::scoped::ScopedFacade::todo_update) scoped to `project` directly
    /// (local-UI path), with the association stated as a resolved [`ScratchpadLink`].
    pub fn todo_update_in(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: TodoDoc,
        scratchpad: ScratchpadLink<ScratchpadId>,
        expected: u64,
    ) -> Result<TodoView, CoordinationError> {
        if let ScratchpadLink::Linked(id) = &scratchpad {
            self.guard_scratchpad(project, Some(*id))?;
        }
        self.emit_todo(
            project,
            self.todos
                .update(project, id, doc, scratchpad, expected)
                .map_err(map_todo_error),
        )
    }

    /// [`todo_complete`](super::scoped::ScopedFacade::todo_complete) scoped to `project` directly (local-UI path).
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

    /// [`todo_set_blockers`](super::scoped::ScopedFacade::todo_set_blockers) scoped to `project` directly (local-UI path).
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

    /// [`todo_add_blocker`](super::scoped::ScopedFacade::todo_add_blocker) scoped to `project` directly (local-UI path).
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

    /// [`todo_remove_blocker`](super::scoped::ScopedFacade::todo_remove_blocker) scoped to `project` directly (local-UI path).
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

    /// Adds a comment `body` to todo `id` in `project` (local-UI path). The local user drives no
    /// bound session, so the comment is left **unattributed** — authorship is the core's call from
    /// the caller's identity, never the caller's to supply, so it can never be forged (an agent's
    /// bound comment carries its label; see [`ScopedFacade::todo_comment_create`](super::scoped::ScopedFacade::todo_comment_create)).
    pub fn todo_comment_create_in(
        &self,
        project: ProjectId,
        id: TodoId,
        body: &str,
    ) -> Result<TodoView, CoordinationError> {
        let created = self
            .todos
            .comment_create(project, id, body, None)?
            .map(|(view, _)| view)
            .ok_or(CoordinationError::UnknownTodo);
        self.emit_todo(project, created)
    }

    /// [`todo_transfer`](super::scoped::ScopedFacade::todo_transfer) scoped to `from`/`to` directly (local-UI path — never
    /// takes a project from an untrusted surface). Moves todo `id` from `from` to `to`, keeping its
    /// comments and completion and clearing its blockers (which referenced source-project ids),
    /// its lock, and its scratchpad association. Emits `TodoChanged` for **both** boards — the source drops it, the target shows it — or
    /// [`CoordinationError::UnknownProject`] if `to` is not loaded (refused before the move, so a
    /// bad target never orphans the todo) / [`CoordinationError::UnknownTodo`] if `from` has none.
    pub fn todo_transfer_in(
        &self,
        from: ProjectId,
        to: ProjectId,
        id: TodoId,
    ) -> Result<TodoView, CoordinationError> {
        if self.projects.get(to)?.is_none() {
            return Err(CoordinationError::UnknownProject);
        }
        let view = self
            .todos
            .transfer(from, to, id)?
            .ok_or(CoordinationError::UnknownTodo)?;
        self.bus
            .publish(DomainEvent::TodoChanged { project: from, id });
        self.bus
            .publish(DomainEvent::TodoChanged { project: to, id });
        Ok(view)
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
    pub(super) fn emit_todo(
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
pub(super) fn map_comment(outcome: CommentOutcome) -> Result<TodoView, CoordinationError> {
    match outcome {
        CommentOutcome::Edited(view) => Ok(*view),
        CommentOutcome::NoTodo => Err(CoordinationError::UnknownTodo),
        CommentOutcome::NoComment => Err(CoordinationError::UnknownComment),
    }
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
