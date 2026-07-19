//! The coordination todo repository — the core [`TodoRepo`] port.
//!
//! One row per todo in the `todos` table, identified durably by the `AUTOINCREMENT` id. The document
//! (title, Markdown body, status) is stored as its JSON in `doc`, and the tag set, blocker ids, and
//! comments as JSON arrays, so the persisted shapes are exactly the domain types and cannot drift. Each
//! state-dependent method holds the single connection guard for its whole operation, so the
//! revision-guarded document write, the tag/blocker/comment read-modify-write, and the conditional
//! lock are atomic — two agents touching one project's todos cannot interleave to clobber an edit or
//! double-grant a lock. A todo is durable and survives an app restart; only its process-owned
//! `locked_by` is cleared on launch ([`clear_locks`](TodoRepo::clear_locks)). The `project_id`
//! foreign key cascades, so removing a project drops its todos.
//!
//! A todo's optional association with the scratchpad it was derived from is the `scratchpad_id`
//! column: only the durable id is stored, so a rename never breaks the link, and its foreign key
//! sets the column NULL when that scratchpad is deleted, so a todo can never point at a document
//! that is gone. Reads resolve the current handle through an outer join, which is why a caller gets
//! a named reference rather than a bare id.

use rusqlite::{Connection, OptionalExtension, Row};
use soloist_core::{
    Comment, CommentAuthor, CommentEdit, ProcessId, ProjectId, ScratchpadId, ScratchpadLink,
    ScratchpadRef, StoreError, StoredTodo, TodoDoc, TodoId, TodoRepo, TodoWriteResult,
};

use crate::{sql_err, SqliteStore};

/// The columns every read selects, in order, so [`row_to_todo`] decodes one shape. Only the
/// scratchpad's id is stored; its current `name` rides along from the join below.
const TODO_COLUMNS: &str = "t.id, t.project_id, t.doc, t.tags, t.blockers, t.comments, \
                            t.locked_by, t.revision, t.scratchpad_id, s.name";

/// The source every read selects from: the todo row with its associated scratchpad's handle
/// resolved. The join is outer because the association is optional, and because a scratchpad the
/// foreign key has not yet unlinked must still read back as an unlinked todo rather than vanish.
const TODO_SOURCE: &str = "todos t LEFT JOIN scratchpads s ON s.id = t.scratchpad_id";

impl TodoRepo for SqliteStore {
    fn create(
        &self,
        project: ProjectId,
        doc: &TodoDoc,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<StoredTodo, StoreError> {
        let doc_json = serialize_doc(doc)?;
        let conn = self.lock();
        conn.execute(
            "INSERT INTO todos (project_id, doc, revision, scratchpad_id) VALUES (?1, ?2, 1, ?3)",
            (project.get() as i64, &doc_json, raw_id(scratchpad)),
        )
        .map_err(sql_err)?;
        let id = TodoId::from_raw(conn.last_insert_rowid() as u64);
        read_one(&conn, project, id)?
            .ok_or_else(|| StoreError::Backend("todo vanished after create".into()))
    }

    fn read(&self, project: ProjectId, id: TodoId) -> Result<Option<StoredTodo>, StoreError> {
        read_one(&self.lock(), project, id)
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredTodo>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {TODO_COLUMNS} FROM {TODO_SOURCE} WHERE t.project_id = ?1 ORDER BY t.id"
            ))
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], row_to_todo)
            .map_err(sql_err)?;
        let mut found = Vec::new();
        for row in rows {
            found.push(row.map_err(sql_err)??);
        }
        Ok(found)
    }

    fn write_doc(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: &TodoDoc,
        scratchpad: ScratchpadLink<ScratchpadId>,
        expected: Option<u64>,
    ) -> Result<TodoWriteResult, StoreError> {
        let doc_json = serialize_doc(doc)?;
        let conn = self.lock();
        // Read the current revision and update under one guard, so the guard check and the write
        // cannot interleave with a concurrent writer.
        let Some(revision) = current_revision(&conn, project, id)? else {
            return Ok(TodoWriteResult::NotFound);
        };
        if let Some(expected) = expected {
            if expected != revision {
                return Ok(TodoWriteResult::Conflict { actual: revision });
            }
        }
        let key = (
            project.get() as i64,
            id.get() as i64,
            &doc_json,
            (revision + 1) as i64,
        );
        // An unchanged association is left out of the statement entirely rather than written back
        // with its own value, so a concurrent link change is not silently reverted by a doc write.
        match stated_link(scratchpad) {
            Some(link) => conn.execute(
                "UPDATE todos SET doc = ?3, revision = ?4, scratchpad_id = ?5
                 WHERE project_id = ?1 AND id = ?2",
                (key.0, key.1, key.2, key.3, link),
            ),
            None => conn.execute(
                "UPDATE todos SET doc = ?3, revision = ?4 WHERE project_id = ?1 AND id = ?2",
                key,
            ),
        }
        .map_err(sql_err)?;
        read_one(&conn, project, id)?
            .map(|stored| TodoWriteResult::Written(Box::new(stored)))
            .ok_or_else(|| StoreError::Backend("todo vanished after write".into()))
    }

    fn delete(&self, project: ProjectId, id: TodoId) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                "DELETE FROM todos WHERE project_id = ?1 AND id = ?2",
                (project.get() as i64, id.get() as i64),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT tags FROM todos WHERE project_id = ?1")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], |row| row.get::<_, String>(0))
            .map_err(sql_err)?;
        let mut distinct = std::collections::BTreeSet::new();
        for row in rows {
            for tag in decode_strings(&row.map_err(sql_err)?)? {
                distinct.insert(tag);
            }
        }
        Ok(distinct.into_iter().collect())
    }

    fn add_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if !todo.tags.iter().any(|existing| existing == tag) {
                todo.tags.push(tag.to_owned());
            }
            todo.tags.sort();
        })
    }

    fn remove_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            todo.tags.retain(|existing| existing != tag)
        })
    }

    fn set_blockers(
        &self,
        project: ProjectId,
        id: TodoId,
        blockers: &[TodoId],
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| todo.blockers = blockers.to_vec())
    }

    fn add_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if !todo.blockers.contains(&blocker) {
                todo.blockers.push(blocker);
            }
        })
    }

    fn remove_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| todo.blockers.retain(|b| *b != blocker))
    }

    fn unmet_blockers(
        &self,
        project: ProjectId,
        blockers: &[TodoId],
    ) -> Result<Vec<TodoId>, StoreError> {
        let conn = self.lock();
        let mut unmet = Vec::new();
        for &blocker in blockers {
            // A blocker that exists in the project and is not yet done is unmet; one that no longer
            // exists is skipped (counts as met, so a deleted blocker never deadlocks a todo).
            if let Some(stored) = read_one(&conn, project, blocker)? {
                if stored.doc.status != soloist_core::TodoStatus::Done {
                    unmet.push(blocker);
                }
            }
        }
        Ok(unmet)
    }

    fn lock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        // "Signals, not ownership": claim the lock only if it is free or already the owner's, leaving
        // another process's lock intact. The returned row reports whoever holds it.
        self.mutate(project, id, |todo| {
            if todo.locked_by.is_none() || todo.locked_by == Some(owner) {
                todo.locked_by = Some(owner);
            }
        })
    }

    fn unlock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if todo.locked_by == Some(owner) {
                todo.locked_by = None;
            }
        })
    }

    fn comment_create(
        &self,
        project: ProjectId,
        id: TodoId,
        body: &str,
        author: Option<CommentAuthor>,
    ) -> Result<Option<(StoredTodo, u64)>, StoreError> {
        let conn = self.lock();
        let Some(mut stored) = read_one(&conn, project, id)? else {
            return Ok(None);
        };
        let comment = stored.comments.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        stored.comments.push(Comment {
            id: comment,
            body: body.to_owned(),
            author,
        });
        write_comments(&conn, project, id, &stored.comments)?;
        Ok(Some((stored, comment)))
    }

    fn comment_update(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<CommentEdit, StoreError> {
        self.edit_comments(project, id, |comments| {
            comments.iter_mut().find(|c| c.id == comment).map(|c| {
                c.body = body.to_owned();
            })
        })
    }

    fn comment_delete(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
    ) -> Result<CommentEdit, StoreError> {
        self.edit_comments(project, id, |comments| {
            let before = comments.len();
            comments.retain(|c| c.id != comment);
            (comments.len() != before).then_some(())
        })
    }

    fn release_owner(&self, process: ProcessId) -> Result<usize, StoreError> {
        self.lock()
            .execute(
                "UPDATE todos SET locked_by = NULL WHERE locked_by = ?1",
                [process.get() as i64],
            )
            .map_err(sql_err)
    }

    fn clear_locks(&self) -> Result<usize, StoreError> {
        self.lock()
            .execute(
                "UPDATE todos SET locked_by = NULL WHERE locked_by IS NOT NULL",
                [],
            )
            .map_err(sql_err)
    }

    fn transfer(
        &self,
        from: ProjectId,
        to: ProjectId,
        id: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        let conn = self.lock();
        // Nothing to move if the todo is not in the source project — read and re-key under one
        // guard so the check and the move cannot interleave.
        if read_one(&conn, from, id)?.is_none() {
            return Ok(None);
        }
        // The id is globally unique (AUTOINCREMENT), so re-keying `project_id` cannot collide.
        // Blockers and the scratchpad association reference source-project rows, and the lock is
        // per-run/process-owned, so all three are cleared; the document, tags, comments, and
        // revision ride along unchanged.
        conn.execute(
            "UPDATE todos SET project_id = ?3, blockers = '[]', locked_by = NULL,
                              scratchpad_id = NULL
             WHERE project_id = ?1 AND id = ?2",
            (from.get() as i64, id.get() as i64, to.get() as i64),
        )
        .map_err(sql_err)?;
        read_one(&conn, to, id)
    }
}

impl SqliteStore {
    /// Reads the todo `(project, id)`, applies `change` to its live columns (tags, blockers,
    /// comments, lock), writes them back, and returns the updated row — all under one connection
    /// guard, so a concurrent change is not lost. `None` if the todo does not exist. The document and
    /// revision are untouched (those are the revision-guarded [`write_doc`] path).
    fn mutate(
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

    /// Applies `edit` to the todo's comment list and persists it; `edit` returns `Some(())` when it
    /// changed a comment and `None` when none matched, mapped to the [`CommentEdit`] outcome.
    fn edit_comments(
        &self,
        project: ProjectId,
        id: TodoId,
        edit: impl FnOnce(&mut Vec<Comment>) -> Option<()>,
    ) -> Result<CommentEdit, StoreError> {
        let conn = self.lock();
        let Some(mut stored) = read_one(&conn, project, id)? else {
            return Ok(CommentEdit::NoTodo);
        };
        match edit(&mut stored.comments) {
            Some(()) => {
                write_comments(&conn, project, id, &stored.comments)?;
                Ok(CommentEdit::Edited(Box::new(stored)))
            }
            None => Ok(CommentEdit::NoComment),
        }
    }
}

/// Writes back the live (non-document) columns of a todo over an already-held guard.
fn write_live(
    conn: &Connection,
    project: ProjectId,
    id: TodoId,
    stored: &StoredTodo,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE todos SET tags = ?3, blockers = ?4, comments = ?5, locked_by = ?6
         WHERE project_id = ?1 AND id = ?2",
        (
            project.get() as i64,
            id.get() as i64,
            serialize_strings(&stored.tags)?,
            serialize_blockers(&stored.blockers)?,
            serialize_comments(&stored.comments)?,
            stored.locked_by.map(|owner| owner.get() as i64),
        ),
    )
    .map_err(sql_err)?;
    Ok(())
}

/// Writes back only the comments column over an already-held guard.
fn write_comments(
    conn: &Connection,
    project: ProjectId,
    id: TodoId,
    comments: &[Comment],
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE todos SET comments = ?3 WHERE project_id = ?1 AND id = ?2",
        (
            project.get() as i64,
            id.get() as i64,
            serialize_comments(comments)?,
        ),
    )
    .map_err(sql_err)?;
    Ok(())
}

/// The current revision of `(project, id)` over an already-held guard, or `None` if absent.
fn current_revision(
    conn: &Connection,
    project: ProjectId,
    id: TodoId,
) -> Result<Option<u64>, StoreError> {
    conn.query_row(
        "SELECT revision FROM todos WHERE project_id = ?1 AND id = ?2",
        (project.get() as i64, id.get() as i64),
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(sql_err)
    .map(|revision| revision.map(|revision| revision as u64))
}

/// One todo by `(project, id)` over an already-held guard, or `None` if absent.
fn read_one(
    conn: &Connection,
    project: ProjectId,
    id: TodoId,
) -> Result<Option<StoredTodo>, StoreError> {
    conn.query_row(
        &format!("SELECT {TODO_COLUMNS} FROM {TODO_SOURCE} WHERE t.project_id = ?1 AND t.id = ?2"),
        (project.get() as i64, id.get() as i64),
        row_to_todo,
    )
    .optional()
    .map_err(sql_err)?
    .transpose()
}

/// Decodes one row into a [`StoredTodo`]. The outer `rusqlite::Result` carries a column error; the
/// inner [`StoreError`] carries a JSON deserialize failure, kept distinct so neither is mistaken for
/// the other.
fn row_to_todo(row: &Row<'_>) -> rusqlite::Result<Result<StoredTodo, StoreError>> {
    let id: i64 = row.get(0)?;
    let project: i64 = row.get(1)?;
    let doc_json: String = row.get(2)?;
    let tags_json: String = row.get(3)?;
    let blockers_json: String = row.get(4)?;
    let comments_json: String = row.get(5)?;
    let locked_by: Option<i64> = row.get(6)?;
    let revision: i64 = row.get(7)?;
    let scratchpad_id: Option<i64> = row.get(8)?;
    let scratchpad_name: Option<String> = row.get(9)?;
    Ok(decode_doc(&doc_json).and_then(|doc| {
        Ok(StoredTodo {
            id: TodoId::from_raw(id as u64),
            project: ProjectId::from_raw(project as u64),
            doc,
            tags: decode_strings(&tags_json)?,
            blockers: decode_blockers(&blockers_json)?,
            comments: decode_comments(&comments_json)?,
            locked_by: locked_by.map(|owner| ProcessId::from_raw(owner as u64)),
            scratchpad: scratchpad_id
                .zip(scratchpad_name)
                .map(|(id, name)| ScratchpadRef {
                    id: ScratchpadId::from_raw(id as u64),
                    name,
                }),
            revision: revision as u64,
        })
    }))
}

/// The raw column value for an optional scratchpad association.
fn raw_id(scratchpad: Option<ScratchpadId>) -> Option<i64> {
    scratchpad.map(|id| id.get() as i64)
}

/// The column value a stated link writes, or `None` when the link is
/// [`Unchanged`](ScratchpadLink::Unchanged) and the column must be left out of the statement.
/// Cleared and linked are both *stated*, so the nesting mirrors the enum's three states exactly.
fn stated_link(link: ScratchpadLink<ScratchpadId>) -> Option<Option<i64>> {
    match link {
        ScratchpadLink::Unchanged => None,
        ScratchpadLink::Cleared => Some(None),
        ScratchpadLink::Linked(id) => Some(Some(id.get() as i64)),
    }
}

/// Serializes a [`TodoDoc`] to the JSON the `doc` column stores.
fn serialize_doc(doc: &TodoDoc) -> Result<String, StoreError> {
    serde_json::to_string(doc).map_err(|err| StoreError::Backend(format!("serialize todo: {err}")))
}

/// Deserializes the `doc` column's JSON into a [`TodoDoc`].
fn decode_doc(json: &str) -> Result<TodoDoc, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo: {err}")))
}

/// Serializes a string list (tags) to the JSON array its column stores.
fn serialize_strings(items: &[String]) -> Result<String, StoreError> {
    serde_json::to_string(items)
        .map_err(|err| StoreError::Backend(format!("serialize todo tags: {err}")))
}

/// Deserializes a JSON string array (tags).
fn decode_strings(json: &str) -> Result<Vec<String>, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo tags: {err}")))
}

/// Serializes the blocker ids to a JSON array of raw ids.
fn serialize_blockers(blockers: &[TodoId]) -> Result<String, StoreError> {
    let raw: Vec<u64> = blockers.iter().map(|id| id.get()).collect();
    serde_json::to_string(&raw)
        .map_err(|err| StoreError::Backend(format!("serialize todo blockers: {err}")))
}

/// Deserializes a JSON array of raw ids into blocker ids.
fn decode_blockers(json: &str) -> Result<Vec<TodoId>, StoreError> {
    let raw: Vec<u64> = serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo blockers: {err}")))?;
    Ok(raw.into_iter().map(TodoId::from_raw).collect())
}

/// Serializes the comment list to the JSON array its column stores.
fn serialize_comments(comments: &[Comment]) -> Result<String, StoreError> {
    serde_json::to_string(comments)
        .map_err(|err| StoreError::Backend(format!("serialize todo comments: {err}")))
}

/// Deserializes a JSON array of comments.
fn decode_comments(json: &str) -> Result<Vec<Comment>, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo comments: {err}")))
}

#[cfg(test)]
#[path = "todos_tests.rs"]
mod tests;
