//! The blocker gate: the aggregate operations that set, add, and remove the todos gating another,
//! and the checks that keep the gate sound.
//!
//! A blocker is a live column beside the todo's document, so changing one carries no revision guard.
//! What the gate mechanically enforces is completion: a todo cannot be marked done while any todo
//! it names is still open.

use super::todo::{TodoError, TodoView, Todos};
use super::todo_repo::StoredTodo;
use crate::ids::{ProjectId, TodoId};
use crate::ports::StoreError;

impl Todos {
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
    pub(super) fn guard_blockers(&self, project: ProjectId, id: TodoId) -> Result<(), TodoError> {
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

    /// Projects a stored row that must exist (the aggregate already checked) to a view, mapping a
    /// concurrent disappearance to [`TodoError::NotFound`].
    fn require(&self, stored: Option<StoredTodo>) -> Result<TodoView, TodoError> {
        match stored {
            Some(stored) => Ok(self.view(stored)?),
            None => Err(TodoError::NotFound),
        }
    }
}
