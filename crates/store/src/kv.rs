//! The coordination kv repository — the core [`KvRepo`] port.
//!
//! One row per `(project, key)` in the `kv` table; the value is stored as JSON text, so the
//! persisted shape is the serialized `serde_json::Value` and cannot drift. `set` is an upsert
//! (`INSERT OR REPLACE`). The `project_id` foreign key cascades, so removing a project drops its
//! kv entries. Kv has no process ownership and is not cleared on launch.

use rusqlite::OptionalExtension;
use serde_json::Value;
use soloist_core::{KvEntry, KvRepo, ProjectId, StoreError};

use crate::{sql_err, SqliteStore};

impl KvRepo for SqliteStore {
    fn set(&self, project: ProjectId, key: &str, value: &Value) -> Result<(), StoreError> {
        let json = value_to_json(value)?;
        self.lock()
            .execute(
                "INSERT OR REPLACE INTO kv (project_id, key, value) VALUES (?1, ?2, ?3)",
                (project.get() as i64, key, &json),
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn get(&self, project: ProjectId, key: &str) -> Result<Option<Value>, StoreError> {
        self.lock()
            .query_row(
                "SELECT value FROM kv WHERE project_id = ?1 AND key = ?2",
                (project.get() as i64, key),
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sql_err)?
            .map(|json| json_to_value(&json))
            .transpose()
    }

    fn delete(&self, project: ProjectId, key: &str) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                "DELETE FROM kv WHERE project_id = ?1 AND key = ?2",
                (project.get() as i64, key),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn list(&self, project: ProjectId) -> Result<Vec<KvEntry>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT key, value FROM kv WHERE project_id = ?1 ORDER BY key")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sql_err)?;
        let mut entries = Vec::new();
        for row in rows {
            let (key, json) = row.map_err(sql_err)?;
            entries.push(KvEntry {
                key,
                value: json_to_value(&json)?,
            });
        }
        Ok(entries)
    }
}

fn value_to_json(value: &Value) -> Result<String, StoreError> {
    serde_json::to_string(value)
        .map_err(|err| StoreError::Backend(format!("serialize kv value: {err}")))
}

fn json_to_value(json: &str) -> Result<Value, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize kv value: {err}")))
}

#[cfg(test)]
#[path = "kv_tests.rs"]
mod tests;
