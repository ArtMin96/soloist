//! The application-settings repository — the core [`SettingsRepo`] port.
//!
//! A single global row (`id = 1`) in the `settings` table; the document is stored as JSON text, so
//! the persisted shape is the serialized [`Settings`] type and cannot drift. `save` is an upsert
//! (`INSERT OR REPLACE`). Settings are global (not project-scoped) and are never cleared on launch.

use rusqlite::OptionalExtension;
use soloist_core::{Settings, SettingsRepo, StoreError};

use crate::{sql_err, SqliteStore};

impl SettingsRepo for SqliteStore {
    fn load(&self) -> Result<Option<Settings>, StoreError> {
        self.lock()
            .query_row("SELECT doc FROM settings WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(sql_err)?
            .map(|json| json_to_settings(&json))
            .transpose()
    }

    fn save(&self, settings: &Settings) -> Result<(), StoreError> {
        let json = settings_to_json(settings)?;
        self.lock()
            .execute(
                "INSERT OR REPLACE INTO settings (id, doc) VALUES (1, ?1)",
                (&json,),
            )
            .map(|_| ())
            .map_err(sql_err)
    }
}

fn settings_to_json(settings: &Settings) -> Result<String, StoreError> {
    serde_json::to_string(settings)
        .map_err(|err| StoreError::Backend(format!("serialize settings: {err}")))
}

fn json_to_settings(json: &str) -> Result<Settings, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize settings: {err}")))
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
