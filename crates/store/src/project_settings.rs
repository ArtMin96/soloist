//! The per-project local-settings repository — the core [`SettingsRepo<ProjectId, ProjectSettings>`]
//! port.
//!
//! One row per project in the `project_settings` table, keyed by `project_id`; the document is
//! stored as JSON text, so the persisted shape is the serialized [`ProjectSettings`] type and
//! cannot drift. `save` is an upsert (`INSERT OR REPLACE`). The `project_id` foreign key cascades,
//! so removing a project drops its local settings; these are durable and never cleared on launch.
//! Stored apart from the project's shared `solo.yml` config (C1) — the two are never merged.

use rusqlite::OptionalExtension;
use soloist_core::{ProjectId, ProjectSettings, SettingsRepo, StoreError};

use crate::{sql_err, SqliteStore};

impl SettingsRepo<ProjectId, ProjectSettings> for SqliteStore {
    fn load(&self, project: &ProjectId) -> Result<Option<ProjectSettings>, StoreError> {
        self.lock()
            .query_row(
                "SELECT doc FROM project_settings WHERE project_id = ?1",
                [project.get() as i64],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sql_err)?
            .map(|json| json_to_settings(&json))
            .transpose()
    }

    fn save(&self, project: &ProjectId, settings: &ProjectSettings) -> Result<(), StoreError> {
        let json = settings_to_json(settings)?;
        self.lock()
            .execute(
                "INSERT OR REPLACE INTO project_settings (project_id, doc) VALUES (?1, ?2)",
                (project.get() as i64, &json),
            )
            .map(|_| ())
            .map_err(sql_err)
    }
}

fn settings_to_json(settings: &ProjectSettings) -> Result<String, StoreError> {
    serde_json::to_string(settings)
        .map_err(|err| StoreError::Backend(format!("serialize project settings: {err}")))
}

fn json_to_settings(json: &str) -> Result<ProjectSettings, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize project settings: {err}")))
}

#[cfg(test)]
#[path = "project_settings_tests.rs"]
mod tests;
