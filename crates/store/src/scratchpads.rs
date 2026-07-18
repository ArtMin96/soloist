//! The coordination scratchpad repository — the core [`ScratchpadRepo`] port.
//!
//! One row per `(project_id, name)` in the `scratchpads` table, identified durably by the
//! `AUTOINCREMENT` id (never reused, so a renamed scratchpad keeps its identity). The free-form
//! Markdown body is stored as-is in the `doc` column and the tag set as a JSON array in `tags`. Each
//! state-dependent method holds the single connection guard for its whole operation, so the
//! revision-guarded write, the tag read-modify-write, and the rename uniqueness check are atomic —
//! two agents editing one project's scratchpads cannot interleave to clobber an edit or duplicate a
//! name. Unlike leases and timers a scratchpad is durable and **not** process-owned: it survives an
//! app restart, so there is no launch-reconcile clear. The `project_id` foreign key cascades, so
//! removing a project drops its scratchpads.

use rusqlite::{Connection, OptionalExtension, Row};
use soloist_core::{
    ProjectId, RenameResult, ScratchpadId, ScratchpadRepo, StoreError, StoredScratchpad,
    TransferResult, WriteResult,
};

use crate::{sql_err, SqliteStore};

/// The columns every read selects, in order, so [`row_to_scratchpad`] decodes one shape.
const SCRATCHPAD_COLUMNS: &str = "id, project_id, name, doc, tags, archived, revision, updated_at";

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
        // Read the current revision and update-or-insert under one guard, so the guard check and the
        // write cannot interleave with a concurrent writer.
        let current = current_revision(&conn, project, name)?;
        match (current, expected) {
            // Update the existing row at the expected revision, bumping it and stamping the write.
            (Some(revision), Some(expected)) if revision == expected => {
                conn.execute(
                    "UPDATE scratchpads SET doc = ?3, revision = ?4, updated_at = ?5
                     WHERE project_id = ?1 AND name = ?2",
                    (
                        project.get() as i64,
                        name,
                        body,
                        (revision + 1) as i64,
                        now as i64,
                    ),
                )
                .map_err(sql_err)?;
                read_one(&conn, project, name)?
                    .map(|stored| WriteResult::Written(Box::new(stored)))
                    .ok_or_else(|| StoreError::Backend("scratchpad vanished after write".into()))
            }
            // Create a fresh row only when none exists and the caller expected absence.
            (None, None) => {
                conn.execute(
                    "INSERT INTO scratchpads (project_id, name, doc, tags, archived, revision, updated_at)
                     VALUES (?1, ?2, ?3, '[]', 0, 1, ?4)",
                    (project.get() as i64, name, body, now as i64),
                )
                .map_err(sql_err)?;
                read_one(&conn, project, name)?
                    .map(|stored| WriteResult::Written(Box::new(stored)))
                    .ok_or_else(|| StoreError::Backend("scratchpad vanished after create".into()))
            }
            // Any other pairing is a revision mismatch; nothing is written.
            (actual, _) => Ok(WriteResult::Conflict { actual }),
        }
    }

    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredScratchpad>, StoreError> {
        read_one(&self.lock(), project, name)
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {SCRATCHPAD_COLUMNS} FROM scratchpads WHERE project_id = ?1 ORDER BY name"
            ))
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], row_to_scratchpad)
            .map_err(sql_err)?;
        let mut found = Vec::new();
        for row in rows {
            found.push(row.map_err(sql_err)??);
        }
        Ok(found)
    }

    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError> {
        let conn = self.lock();
        // Reject a taken target before the update (a clearer outcome than the UNIQUE violation), and
        // do both under one guard so two renames cannot both take one name.
        if from != to && current_revision(&conn, project, to)?.is_some() {
            return Ok(RenameResult::NameTaken);
        }
        let updated = conn
            .execute(
                "UPDATE scratchpads SET name = ?3 WHERE project_id = ?1 AND name = ?2",
                (project.get() as i64, from, to),
            )
            .map_err(sql_err)?;
        if updated == 0 {
            return Ok(RenameResult::NotFound);
        }
        read_one(&conn, project, to)?
            .map(|stored| RenameResult::Renamed(Box::new(stored)))
            .ok_or_else(|| StoreError::Backend("scratchpad vanished after rename".into()))
    }

    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        self.update_tags(project, name, |current| {
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
        self.update_tags(project, name, |current| {
            current.retain(|tag| !tags.contains(tag))
        })
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT tags FROM scratchpads WHERE project_id = ?1")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], |row| row.get::<_, String>(0))
            .map_err(sql_err)?;
        let mut distinct = std::collections::BTreeSet::new();
        for row in rows {
            for tag in decode_tags(&row.map_err(sql_err)?)? {
                distinct.insert(tag);
            }
        }
        Ok(distinct.into_iter().collect())
    }

    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let conn = self.lock();
        let updated = conn
            .execute(
                "UPDATE scratchpads SET archived = ?3 WHERE project_id = ?1 AND name = ?2",
                (project.get() as i64, name, archived as i64),
            )
            .map_err(sql_err)?;
        if updated == 0 {
            return Ok(None);
        }
        read_one(&conn, project, name)
    }

    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                "DELETE FROM scratchpads WHERE project_id = ?1 AND name = ?2",
                (project.get() as i64, name),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<TransferResult, StoreError> {
        let conn = self.lock();
        // Reject a name already used in the target before the update (clearer than the UNIQUE
        // violation), and do both under one guard so a move and a create cannot both take the name.
        if current_revision(&conn, to, name)?.is_some() {
            return Ok(TransferResult::NameTaken);
        }
        // Re-key the project only; the durable id, name, document, tags, archived flag, and revision
        // all ride along unchanged.
        let updated = conn
            .execute(
                "UPDATE scratchpads SET project_id = ?3 WHERE project_id = ?1 AND name = ?2",
                (from.get() as i64, name, to.get() as i64),
            )
            .map_err(sql_err)?;
        if updated == 0 {
            return Ok(TransferResult::NotFound);
        }
        read_one(&conn, to, name)?
            .map(|stored| TransferResult::Transferred(Box::new(stored)))
            .ok_or_else(|| StoreError::Backend("scratchpad vanished after transfer".into()))
    }
}

impl SqliteStore {
    /// Reads the scratchpad's tag set, applies `change`, writes it back, and returns the updated
    /// row — all under one connection guard, so a concurrent tag change is not lost. `None` if the
    /// scratchpad does not exist. The tag set is stored sorted, normalized here after every change
    /// so add and remove leave the same canonical order.
    fn update_tags(
        &self,
        project: ProjectId,
        name: &str,
        change: impl FnOnce(&mut Vec<String>),
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let conn = self.lock();
        let Some(mut stored) = read_one(&conn, project, name)? else {
            return Ok(None);
        };
        change(&mut stored.tags);
        stored.tags.sort();
        let tags_json = serialize_tags(&stored.tags)?;
        conn.execute(
            "UPDATE scratchpads SET tags = ?3 WHERE project_id = ?1 AND name = ?2",
            (project.get() as i64, name, &tags_json),
        )
        .map_err(sql_err)?;
        Ok(Some(stored))
    }
}

/// The current revision of `(project, name)` over an already-held guard, or `None` if absent.
fn current_revision(
    conn: &Connection,
    project: ProjectId,
    name: &str,
) -> Result<Option<u64>, StoreError> {
    conn.query_row(
        "SELECT revision FROM scratchpads WHERE project_id = ?1 AND name = ?2",
        (project.get() as i64, name),
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(sql_err)
    .map(|revision| revision.map(|revision| revision as u64))
}

/// One scratchpad by `(project, name)` over an already-held guard, or `None` if absent.
fn read_one(
    conn: &Connection,
    project: ProjectId,
    name: &str,
) -> Result<Option<StoredScratchpad>, StoreError> {
    conn.query_row(
        &format!(
            "SELECT {SCRATCHPAD_COLUMNS} FROM scratchpads WHERE project_id = ?1 AND name = ?2"
        ),
        (project.get() as i64, name),
        row_to_scratchpad,
    )
    .optional()
    .map_err(sql_err)?
    .transpose()
}

/// Decodes one row into a [`StoredScratchpad`]. The outer `rusqlite::Result` carries a column error;
/// the inner [`StoreError`] carries a `tags` JSON deserialize failure, kept distinct so neither is
/// mistaken for the other. The `doc` column is the raw Markdown body, read as-is.
fn row_to_scratchpad(row: &Row<'_>) -> rusqlite::Result<Result<StoredScratchpad, StoreError>> {
    let id: i64 = row.get(0)?;
    let project: i64 = row.get(1)?;
    let name: String = row.get(2)?;
    let body: String = row.get(3)?;
    let tags_json: String = row.get(4)?;
    let archived: i64 = row.get(5)?;
    let revision: i64 = row.get(6)?;
    let updated_at: i64 = row.get(7)?;
    Ok(decode_tags(&tags_json).map(|tags| StoredScratchpad {
        id: ScratchpadId::from_raw(id as u64),
        project: ProjectId::from_raw(project as u64),
        name,
        body,
        tags,
        archived: archived != 0,
        revision: revision as u64,
        updated_at: updated_at as u64,
    }))
}

/// Serializes a tag set to the JSON array the `tags` column stores.
fn serialize_tags(tags: &[String]) -> Result<String, StoreError> {
    serde_json::to_string(tags)
        .map_err(|err| StoreError::Backend(format!("serialize scratchpad tags: {err}")))
}

/// Deserializes the `tags` column's JSON array into a tag set.
fn decode_tags(json: &str) -> Result<Vec<String>, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize scratchpad tags: {err}")))
}

#[cfg(test)]
#[path = "scratchpads_tests.rs"]
mod tests;
