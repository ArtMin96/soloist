//! Comments on a todo: who wrote one, what a comment is, and the aggregate operations that create,
//! edit, delete, and list them.
//!
//! Comments are live state beside the todo's document, not part of it, so a comment change never
//! collides with a concurrent specification edit and carries no revision guard.

use serde::{Deserialize, Serialize};

use super::todo::{TodoView, Todos};
use super::todo_repo::CommentEdit;
use crate::ids::{ProcessId, ProjectId, TodoId};
use crate::ports::StoreError;

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

/// The outcome of a comment edit through the aggregate: the todo's updated view, no todo under that
/// id, or no comment under that id within the todo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentOutcome {
    Edited(Box<TodoView>),
    NoTodo,
    NoComment,
}

impl Todos {
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

    /// Translates a repo [`CommentEdit`] to the aggregate's [`CommentOutcome`], building the view.
    fn comment_outcome(&self, edit: CommentEdit) -> Result<CommentOutcome, StoreError> {
        Ok(match edit {
            CommentEdit::Edited(stored) => CommentOutcome::Edited(Box::new(self.view(*stored)?)),
            CommentEdit::NoTodo => CommentOutcome::NoTodo,
            CommentEdit::NoComment => CommentOutcome::NoComment,
        })
    }
}
