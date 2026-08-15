use rusqlite::{Connection, OptionalExtension};
use soloist_core::{AgentTool, StoreError};

use crate::sql_err;

/// Whether a table of `name` exists — used by guarded renames so they stay no-ops on a re-run.
pub(super) fn table_exists(conn: &Connection, name: &str) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .map(|found| found.is_some())
    .map_err(sql_err)
}

/// Whether `table` has a column named `column`, keeping guarded column additions idempotent.
pub(super) fn column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<bool, StoreError> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(sql_err)?;
    let mut names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sql_err)?;
    names
        .try_fold(false, |found, name| Ok(found || name? == column))
        .map_err(sql_err)
}

/// Seeds the built-in agent providers into a fresh `agent_tools` table without replacing edits.
pub(super) fn seed_builtin_agent_tools(conn: &Connection) -> Result<(), StoreError> {
    for (position, tool) in AgentTool::builtin_defaults().iter().enumerate() {
        let definition = serde_json::to_string(tool)
            .map_err(|err| StoreError::Backend(format!("serialize agent tool: {err}")))?;
        conn.execute(
            "INSERT OR IGNORE INTO agent_tools (name, position, definition) VALUES (?1, ?2, ?3)",
            (&tool.name, position as i64, &definition),
        )
        .map_err(sql_err)?;
    }
    Ok(())
}
