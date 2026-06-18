//! Versioned, idempotent SQLite migrations for the durable store.

use rusqlite::Connection;
use soloist_core::StoreError;

use crate::sql_err;

/// The newest schema version this build knows how to migrate to.
pub(crate) const SCHEMA_VERSION: i64 = 2;

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

    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(sql_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
