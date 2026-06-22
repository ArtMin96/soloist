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
#[path = "agents_tests.rs"]
mod tests;
