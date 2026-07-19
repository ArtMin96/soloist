//! How a todo row is addressed, read, and written: the column list every read shares, the
//! statements that run over an already-held connection guard, and the decode of a row into a
//! [`StoredTodo`].
//!
//! Every statement here takes the guard the caller already holds, so a repository operation can
//! compose several of them into one atomic step.

use rusqlite::{Connection, OptionalExtension, Row};
use soloist_core::{
    Comment, ProcessId, ProjectId, ScratchpadId, ScratchpadLink, ScratchpadRef, StoreError,
    StoredTodo, TodoId,
};

use crate::sql_err;
use crate::todo_json::{
    decode_blockers, decode_comments, decode_doc, decode_strings, serialize_blockers,
    serialize_comments, serialize_strings,
};

/// The columns every read selects, in order, so [`row_to_todo`] decodes one shape. Only the
/// scratchpad's id is stored; its current `name` rides along from the join below.
pub(crate) const TODO_COLUMNS: &str = "t.id, t.project_id, t.doc, t.tags, t.blockers, t.comments, \
                                       t.locked_by, t.revision, t.scratchpad_id, s.name";

/// The source every read selects from: the todo row with its associated scratchpad's handle
/// resolved. The join is outer because the association is optional, and because a scratchpad the
/// foreign key has not yet unlinked must still read back as an unlinked todo rather than vanish.
pub(crate) const TODO_SOURCE: &str = "todos t LEFT JOIN scratchpads s ON s.id = t.scratchpad_id";

/// Writes back the live (non-document) columns of a todo over an already-held guard.
pub(crate) fn write_live(
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
pub(crate) fn write_comments(
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
pub(crate) fn current_revision(
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
pub(crate) fn read_one(
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
pub(crate) fn row_to_todo(row: &Row<'_>) -> rusqlite::Result<Result<StoredTodo, StoreError>> {
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

/// Moves every todo in `from` derived from `scratchpad` into `to` over an already-held guard,
/// returning the moved ids in id order — the cascade a scratchpad transfer runs inside its
/// transaction, so the document and the work derived from it never half-move.
///
/// The association is deliberately **kept**: both ends move, so it still resolves within the target
/// project. The process-owned lock is dropped and blockers are filtered to the todos that move too,
/// exactly as a cross-project todo transfer treats them — a blocker left behind names a row in
/// another project.
pub(crate) fn transfer_derived(
    conn: &Connection,
    from: ProjectId,
    to: ProjectId,
    scratchpad: ScratchpadId,
) -> Result<Vec<TodoId>, StoreError> {
    let derived = {
        let mut stmt = conn
            .prepare(
                "SELECT id, blockers FROM todos
                 WHERE project_id = ?1 AND scratchpad_id = ?2 ORDER BY id",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map((from.get() as i64, scratchpad.get() as i64), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err)?;
        let mut derived = Vec::new();
        for row in rows {
            let (id, blockers) = row.map_err(sql_err)?;
            derived.push((TodoId::from_raw(id as u64), blockers));
        }
        derived
    };
    let moved: Vec<TodoId> = derived.iter().map(|(id, _)| *id).collect();
    for (id, blockers) in &derived {
        let kept: Vec<TodoId> = decode_blockers(blockers)?
            .into_iter()
            .filter(|blocker| moved.contains(blocker))
            .collect();
        conn.execute(
            "UPDATE todos SET project_id = ?3, blockers = ?4, locked_by = NULL
             WHERE project_id = ?1 AND id = ?2",
            (
                from.get() as i64,
                id.get() as i64,
                to.get() as i64,
                serialize_blockers(&kept)?,
            ),
        )
        .map_err(sql_err)?;
    }
    Ok(moved)
}

/// The raw column value for an optional scratchpad association.
pub(crate) fn raw_id(scratchpad: Option<ScratchpadId>) -> Option<i64> {
    scratchpad.map(|id| id.get() as i64)
}

/// The column value a stated link writes, or `None` when the link is
/// [`Unchanged`](ScratchpadLink::Unchanged) and the column must be left out of the statement.
/// Cleared and linked are both *stated*, so the nesting mirrors the enum's three states exactly.
pub(crate) fn stated_link(link: ScratchpadLink<ScratchpadId>) -> Option<Option<i64>> {
    match link {
        ScratchpadLink::Unchanged => None,
        ScratchpadLink::Cleared => Some(None),
        ScratchpadLink::Linked(id) => Some(Some(id.get() as i64)),
    }
}
