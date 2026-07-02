//! Versioned, idempotent SQLite migrations for the durable store.

use rusqlite::Connection;
use soloist_core::{AgentTool, StoreError};

use crate::sql_err;

/// The newest schema version this build knows how to migrate to.
pub(crate) const SCHEMA_VERSION: i64 = 11;

/// Applies migrations newer than the database's recorded `user_version`. Each step
/// is idempotent; the version is bumped only after all pending steps succeed. A
/// database whose version is newer than this build understands is refused rather
/// than silently downgraded.
pub(crate) fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sql_err)?;

    if version > SCHEMA_VERSION {
        return Err(StoreError::Backend(format!(
            "database schema version {version} is newer than this build supports \
             ({SCHEMA_VERSION}); upgrade Soloist"
        )));
    }

    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 2 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                 id   INTEGER PRIMARY KEY,
                 root TEXT NOT NULL UNIQUE,
                 name TEXT,
                 icon TEXT
             );
             CREATE TABLE IF NOT EXISTS trust (
                 project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 variant_hash TEXT NOT NULL,
                 PRIMARY KEY (project_id, variant_hash)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_tools (
                 name       TEXT PRIMARY KEY,
                 position   INTEGER NOT NULL,
                 definition TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
        seed_builtin_agent_tools(conn)?;
    }

    if version < 4 {
        // Coordination leases: one row per (project, key). `owner` is a per-run process id and
        // the millis are a persistable wall clock. The project foreign key cascades, so removing a
        // project drops its leases; launch reconciliation clears whatever a previous run left.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS leases (
                 project_id           INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 key                  TEXT NOT NULL,
                 owner                INTEGER NOT NULL,
                 acquired_unix_millis INTEGER NOT NULL,
                 expires_unix_millis  INTEGER NOT NULL,
                 PRIMARY KEY (project_id, key)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 5 {
        // Coordination timers: one row per timer, with a store-assigned id that is never reused
        // (`AUTOINCREMENT`). `fire` is the JSON of the domain `FireCond` (kind + watched set), so
        // the persisted shape cannot drift; `deadline_unix_millis` is its own column so a pause can
        // freeze it. `paused`/`remaining_millis` carry the suspended state. The project foreign key
        // cascades; launch reconciliation clears whatever a previous run left (per-run owner ids).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS timers (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id           INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 owner                INTEGER NOT NULL,
                 body                 TEXT NOT NULL,
                 fire                 TEXT NOT NULL,
                 deadline_unix_millis INTEGER NOT NULL,
                 paused               INTEGER NOT NULL DEFAULT 0,
                 remaining_millis     INTEGER
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 6 {
        // Coordination scratchpads: durable, project-scoped shared documents. The store-assigned
        // `id` is durable and never reused (`AUTOINCREMENT`); `doc` is the JSON of the disciplined
        // `ScratchpadDoc` and `tags` a JSON array, so the persisted shape cannot drift; `revision`
        // guards optimistic-concurrency writes; `(project_id, name)` is unique (the addressing
        // handle). Unlike leases and timers these are NOT process-owned and are NOT cleared on
        // launch — a scratchpad survives an app restart. The project foreign key cascades.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scratchpads (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 name       TEXT NOT NULL,
                 doc        TEXT NOT NULL,
                 tags       TEXT NOT NULL,
                 archived   INTEGER NOT NULL DEFAULT 0,
                 revision   INTEGER NOT NULL,
                 UNIQUE (project_id, name)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 7 {
        // Coordination todos: durable, project-scoped work items. The store-assigned `id` is durable
        // and never reused (`AUTOINCREMENT`); `doc` is the JSON of the disciplined `TodoDoc` (title,
        // description, acceptance criteria, risks, status), and `tags`/`blockers`/`comments` are JSON
        // arrays, so the persisted shapes cannot drift; `revision` guards optimistic-concurrency doc
        // writes. `locked_by` is the per-run process id holding the todo's lock, or NULL — the only
        // process-owned, per-run field, cleared on launch (the todo itself survives, G11). The
        // project foreign key cascades, so removing a project drops its todos.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 doc        TEXT NOT NULL,
                 tags       TEXT NOT NULL DEFAULT '[]',
                 blockers   TEXT NOT NULL DEFAULT '[]',
                 comments   TEXT NOT NULL DEFAULT '[]',
                 locked_by  INTEGER,
                 revision   INTEGER NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 8 {
        // Coordination kv: one row per (project, key). `value` stores arbitrary JSON — the TEXT
        // column holds the serialized form; parsing happens at the repository boundary. Durable,
        // not process-owned, and survives an app restart; no launch-reconcile clear. The project
        // foreign key cascades, so removing a project drops its kv entries.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv (
                 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 key        TEXT NOT NULL,
                 value      TEXT NOT NULL,
                 PRIMARY KEY (project_id, key)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 9 {
        // Application settings: a single global record (the `id = 1` CHECK enforces the singleton),
        // not project-scoped, so it has no project foreign key and survives across projects. `doc`
        // is the JSON of the `Settings` document, so the persisted shape is the domain type and
        // cannot drift; serde defaults fill any field a newer build adds. Durable, never cleared on
        // launch.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                 id  INTEGER PRIMARY KEY CHECK (id = 1),
                 doc TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 10 {
        // Per-project local settings: one row per project, keyed by `project_id`, holding the
        // `ProjectSettings` document as JSON so the persisted shape is the domain type and cannot
        // drift (serde defaults fill any field a newer build adds). The project foreign key
        // cascades, so removing a project drops its local settings; durable, never cleared on
        // launch. Stored apart from the project's shared `solo.yml` config (C1) — never merged.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_settings (
                 project_id INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
                 doc        TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 11 {
        // Feedback: append-only local notes about Soloist itself. Global (no project foreign
        // key — feedback outlives any project) and never transmitted anywhere; the user reads
        // it back at their own pace. The store-assigned `id` is durable and never reused
        // (`AUTOINCREMENT`); the millis are a persistable wall clock.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS feedback (
                 id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                 message               TEXT NOT NULL,
                 submitted_unix_millis INTEGER NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(sql_err)?;
    }
    Ok(())
}

/// Seeds the built-in agent providers into a fresh `agent_tools` table, preserving their
/// canonical order via `position`. The definition is the tool's JSON, so the persisted shape
/// is exactly the domain type and cannot drift from it. `INSERT OR IGNORE` keeps the step
/// idempotent and never clobbers a tool the user has since edited under the same name.
fn seed_builtin_agent_tools(conn: &Connection) -> Result<(), StoreError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_a_fresh_database_to_the_current_schema() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        migrate(&conn).expect("a fresh database migrates cleanly");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version");
        assert_eq!(
            version, SCHEMA_VERSION,
            "migration must advance a fresh database to the current schema version"
        );

        for table in [
            "meta",
            "projects",
            "trust",
            "agent_tools",
            "leases",
            "timers",
            "scratchpads",
            "todos",
            "kv",
            "settings",
            "project_settings",
            "feedback",
        ] {
            let exists = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |_| Ok(()),
                )
                .is_ok();
            assert!(exists, "migration must create the `{table}` table");
        }

        // The built-in agent providers are seeded on the fresh database.
        let seeded: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_tools", [], |row| row.get(0))
            .expect("count seeded agent tools");
        assert_eq!(
            seeded,
            AgentTool::builtin_defaults().len() as i64,
            "migration must seed the built-in agent providers"
        );

        // Re-running over an already-current database touches nothing (idempotent steps).
        migrate(&conn).expect("re-running migrate on a current database is a no-op");
    }

    #[test]
    fn refuses_a_schema_newer_than_this_build() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .expect("set a future schema version");
        assert!(
            migrate(&conn).is_err(),
            "a newer schema must be refused, not silently downgraded"
        );
    }
}
