use soloist_core::{Comment, CommentEdit, ProjectId, StoreError, StoredTodo, TodoId};

use crate::todo_rows::{read_one, write_comments, write_live};
use crate::SqliteStore;

impl SqliteStore {
    /// Reads the todo `(project, id)`, applies `change` to its live columns, and writes them back
    /// under one connection guard. The revision-guarded document fields are untouched.
    pub(super) fn mutate(
        &self,
        project: ProjectId,
        id: TodoId,
        change: impl FnOnce(&mut StoredTodo),
    ) -> Result<Option<StoredTodo>, StoreError> {
        let conn = self.lock();
        let Some(mut stored) = read_one(&conn, project, id)? else {
            return Ok(None);
        };
        change(&mut stored);
        write_live(&conn, project, id, &stored)?;
        Ok(Some(stored))
    }

    /// Applies `edit` to a todo's comment list and maps whether it matched to [`CommentEdit`].
    pub(super) fn edit_comments(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
        edit: impl FnOnce(&mut Vec<Comment>) -> Option<()>,
    ) -> Result<CommentEdit, StoreError> {
        let conn = self.lock();
        let Some(mut stored) = read_one(&conn, project, id)? else {
            return Ok(CommentEdit::NoTodo);
        };
        if stored
            .completion
            .as_ref()
            .is_some_and(|completion| completion.comment() == comment)
        {
            return Ok(CommentEdit::CompletionProtected);
        }
        match edit(&mut stored.comments) {
            Some(()) => {
                write_comments(&conn, project, id, &stored.comments)?;
                Ok(CommentEdit::Edited(Box::new(stored)))
            }
            None => Ok(CommentEdit::NoComment),
        }
    }
}
