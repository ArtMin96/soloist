//! The agent-tool repository — the core [`AgentToolRepo`] port.
//!
//! Each row stores a tool's definition as its JSON, so the persisted shape is exactly the
//! domain type and cannot drift from a hand-maintained column set. The built-in providers
//! are seeded by the migration ([`crate::migrate`]); `position` preserves their canonical
//! order. Listing the registry is read-only; editing tools lands with the agent-launch flow.

use soloist_core::{AgentTool, AgentToolRepo, StoreError};

use crate::{sql_err, SqliteStore};

impl AgentToolRepo for SqliteStore {
    fn list(&self) -> Result<Vec<AgentTool>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT definition FROM agent_tools ORDER BY position, name")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sql_err)?;

        let mut tools = Vec::new();
        for row in rows {
            let definition = row.map_err(sql_err)?;
            let tool = serde_json::from_str(&definition)
                .map_err(|err| StoreError::Backend(format!("deserialize agent tool: {err}")))?;
            tools.push(tool);
        }
        Ok(tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteStore;
    use tempfile::tempdir;

    #[test]
    fn list_returns_the_seeded_builtin_providers_in_order() {
        let store = SqliteStore::open_in_memory().expect("open");
        assert_eq!(
            store.list().expect("list"),
            AgentTool::builtin_defaults(),
            "a fresh store lists exactly the seeded built-in providers, in order"
        );
    }

    #[test]
    fn agent_tools_persist_across_reopen_without_reseeding() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let count = {
            let store = SqliteStore::open(&db).expect("open");
            store.list().expect("list").len()
        };
        assert_eq!(count, AgentTool::builtin_defaults().len());

        // Reopening re-runs migrate (a no-op at the current version), so the seed is not
        // duplicated — the registry is stable across restarts.
        let reopened = SqliteStore::open(&db).expect("reopen");
        assert_eq!(
            reopened.list().expect("list").len(),
            AgentTool::builtin_defaults().len(),
            "reopening must not re-seed the built-in providers"
        );
    }
}
