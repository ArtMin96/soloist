//! The coordination scratchpad repository — the core [`ScratchpadRepo`] port.
//!
//! One row per `(project_id, name)` in the `scratchpads` table, identified durably by the
//! `AUTOINCREMENT` id (never reused, so a renamed scratchpad keeps its identity). The free-form
//! Markdown body is stored as-is in the `doc` column and the tag set as a JSON array in `tags`.
//! Unlike leases and timers a scratchpad is durable and **not** process-owned: it survives an app
//! restart, so there is no launch-reconcile clear. The `project_id` foreign key cascades, so
//! removing a project drops its scratchpads.
//!
//! A cross-project transfer is the one operation reaching beyond this table: the todos derived
//! from the moved scratchpad go with it, keeping their association, so it runs inside a
//! transaction and the two tables can never half-move.
//!
//! Every other operation's guard, conflict, decode, and tag read-modify-write mechanics are
//! shared with [`diagrams`](crate::diagrams) over [`doc_table`](crate::doc_table); this module
//! keeps only this table's statements, its row shape, and the transfer that only scratchpads
//! support.

use rusqlite::Row;
use soloist_core::{
    ProjectId, RenameResult, ScratchpadId, ScratchpadRepo, StoreError, StoredScratchpad,
    TransferResult, TransferredScratchpad, WriteResult,
};

use crate::doc_table::{self, DocRename, DocStatements, DocTable, DocWrite, TaggedRow};
use crate::todo_rows::transfer_derived;
use crate::{sql_err, SqliteStore};

/// The noun a "row vanished" error and a tag (de)serialize error name for this table.
const LABEL: &str = "scratchpad";

static STATEMENTS: DocStatements = DocStatements {
    label: LABEL,
    select_revision: "SELECT revision FROM scratchpads WHERE project_id = ?1 AND name = ?2",
    select_one: "SELECT id, project_id, name, doc, tags, archived, revision, updated_at \
                 FROM scratchpads WHERE project_id = ?1 AND name = ?2",
    select_list: "SELECT id, project_id, name, doc, tags, archived, revision, updated_at \
                  FROM scratchpads WHERE project_id = ?1 ORDER BY name",
    select_tags: "SELECT tags FROM scratchpads WHERE project_id = ?1",
    select_contains: "SELECT 1 FROM scratchpads WHERE project_id = ?1 AND id = ?2",
    insert: "INSERT INTO scratchpads (project_id, name, doc, tags, archived, revision, \
             updated_at) VALUES (?1, ?2, ?3, '[]', 0, 1, ?4)",
    update_body: "UPDATE scratchpads SET doc = ?3, revision = ?4, updated_at = ?5 \
                  WHERE project_id = ?1 AND name = ?2",
    update_name: "UPDATE scratchpads SET name = ?3 WHERE project_id = ?1 AND name = ?2",
    update_tags: "UPDATE scratchpads SET tags = ?3 WHERE project_id = ?1 AND name = ?2",
    update_archived: "UPDATE scratchpads SET archived = ?3 WHERE project_id = ?1 AND name = ?2",
    delete: "DELETE FROM scratchpads WHERE project_id = ?1 AND name = ?2",
};

fn table() -> DocTable<'static, StoredScratchpad> {
    DocTable::new(&STATEMENTS, row_to_scratchpad)
}

impl TaggedRow for StoredScratchpad {
    fn tags_mut(&mut self) -> &mut Vec<String> {
        &mut self.tags
    }
}

impl ScratchpadRepo for SqliteStore {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        body: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<WriteResult, StoreError> {
        let conn = self.lock();
        match table().write(&conn, project, name, body, expected, now)? {
            DocWrite::Written(stored) => Ok(WriteResult::Written(stored)),
            DocWrite::Conflict { actual } => Ok(WriteResult::Conflict { actual }),
        }
    }

    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredScratchpad>, StoreError> {
        table().read_one(&self.lock(), project, name)
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError> {
        table().list(&self.lock(), project)
    }

    fn contains(&self, project: ProjectId, id: ScratchpadId) -> Result<bool, StoreError> {
        table().contains(&self.lock(), project, id.get())
    }

    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError> {
        let conn = self.lock();
        match table().rename(&conn, project, from, to)? {
            DocRename::Renamed(stored) => Ok(RenameResult::Renamed(stored)),
            DocRename::NotFound => Ok(RenameResult::NotFound),
            DocRename::NameTaken => Ok(RenameResult::NameTaken),
        }
    }

    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        table().update_tags(&self.lock(), project, name, |current| {
            for tag in tags {
                if !current.contains(tag) {
                    current.push(tag.clone());
                }
            }
        })
    }

    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        table().update_tags(&self.lock(), project, name, |current| {
            current.retain(|tag| !tags.contains(tag))
        })
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        table().tags(&self.lock(), project)
    }

    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        table().set_archived(&self.lock(), project, name, archived)
    }

    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        table().delete(&self.lock(), project, name)
    }

    fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<TransferResult, StoreError> {
        let conn = self.lock();
        // The document and the todos derived from it move as one — a transaction, so a failure
        // part-way leaves neither moved rather than stranding todos from their source.
        let tx = conn.unchecked_transaction().map_err(sql_err)?;
        let table = table();
        // Reject a name already used in the target before the update (clearer than the UNIQUE
        // violation), and do both under one guard so a move and a create cannot both take the name.
        if table.current_revision(&tx, to, name)?.is_some() {
            return Ok(TransferResult::NameTaken);
        }
        let Some(stored) = table.read_one(&tx, from, name)? else {
            return Ok(TransferResult::NotFound);
        };
        // Re-key the project only; the durable id, name, document, tags, archived flag, and
        // revision all ride along unchanged.
        tx.execute(
            "UPDATE scratchpads SET project_id = ?3 WHERE project_id = ?1 AND name = ?2",
            (from.get() as i64, name, to.get() as i64),
        )
        .map_err(sql_err)?;
        let todos = transfer_derived(&tx, from, to, stored.id)?;
        let scratchpad = table
            .read_one(&tx, to, name)?
            .ok_or_else(|| doc_table::vanished(LABEL, "transfer"))?;
        tx.commit().map_err(sql_err)?;
        Ok(TransferResult::Transferred(Box::new(
            TransferredScratchpad { scratchpad, todos },
        )))
    }
}

/// Decodes one row into a [`StoredScratchpad`]. The `doc` column is the raw Markdown body, read
/// as-is.
fn row_to_scratchpad(row: &Row<'_>) -> rusqlite::Result<Result<StoredScratchpad, StoreError>> {
    let id: i64 = row.get(0)?;
    let project: i64 = row.get(1)?;
    let name: String = row.get(2)?;
    let body: String = row.get(3)?;
    let tags_json: String = row.get(4)?;
    let archived: i64 = row.get(5)?;
    let revision: i64 = row.get(6)?;
    let updated_at: i64 = row.get(7)?;
    Ok(
        doc_table::decode_tags(&tags_json, LABEL).map(|tags| StoredScratchpad {
            id: ScratchpadId::from_raw(id as u64),
            project: ProjectId::from_raw(project as u64),
            name,
            body,
            tags,
            archived: archived != 0,
            revision: revision as u64,
            updated_at: updated_at as u64,
        }),
    )
}

#[cfg(test)]
#[path = "scratchpads_tests.rs"]
mod tests;
