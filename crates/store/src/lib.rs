//! SQLite-backed implementation of the core's [`Store`] port.
//!
//! The walking skeleton proves the storage thread with a single `meta` key/value
//! table behind a versioned migration, opened in WAL mode in the app data directory.
//! Later phases grow focused repositories (trust, projects, todos, scratchpads, …)
//! on this same connection following the repository pattern. SQLite is bundled, so
//! the binary carries its own engine and needs no system `libsqlite3`.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use soloist_core::{Store, StoreError};

/// The newest schema version this build knows how to migrate to.
const SCHEMA_VERSION: i64 = 1;

/// A durable store backed by a single SQLite connection.
///
/// `rusqlite::Connection` is `Send` but not `Sync`, so it is guarded by a `Mutex` to
/// satisfy the `Send + Sync` [`Store`] contract. Critical sections are tiny
/// (single-statement metadata reads/writes) and never held across an `await`.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Opens (creating if needed) the database at `path`, enabling WAL and running
    /// pending migrations. Parent directories are created as needed.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let conn = Connection::open(path).map_err(sql_err)?;
        // WAL keeps readers from blocking the writer over a long-running session.
        let _mode: String = conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .map_err(sql_err)?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(sql_err)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Opens the database at the resolved app data directory (see [`data_dir`]).
    pub fn open_default() -> Result<Self, StoreError> {
        let path = data_dir()?.join("soloist.db");
        Self::open(&path)
    }

    /// Opens an ephemeral in-memory database (migrated, but not durable). Used as a
    /// graceful fallback when the durable location is unavailable, so the app stays
    /// usable rather than failing to launch.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(sql_err)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Locks the connection, recovering the guard if a previous holder panicked.
    fn lock(&self) -> MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Store for SqliteStore {
    fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT value FROM meta WHERE key = ?1")
            .map_err(sql_err)?;
        match stmt.query_row([key], |row| row.get::<_, String>(0)) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(sql_err(err)),
        }
    }

    fn meta_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (key, value),
            )
            .map(|_| ())
            .map_err(sql_err)
    }
}

/// Applies any migrations newer than the database's recorded `user_version`. Each
/// step is idempotent and the version is bumped only after it succeeds.
fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sql_err)?;

    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(sql_err)?;
    Ok(())
}

/// Resolves the app data directory: `SOLOIST_APP_DATA_DIR`, else `$XDG_DATA_HOME/soloist`,
/// else `$HOME/.local/share/soloist`.
pub fn data_dir() -> Result<PathBuf, StoreError> {
    if let Some(dir) = std::env::var_os("SOLOIST_APP_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("soloist"));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/share/soloist"));
    }
    Err(StoreError::Backend(
        "cannot resolve data directory: neither SOLOIST_APP_DATA_DIR nor HOME is set".into(),
    ))
}

fn sql_err(err: rusqlite::Error) -> StoreError {
    StoreError::Backend(err.to_string())
}

fn io_err(err: std::io::Error) -> StoreError {
    StoreError::Backend(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn opens_with_wal_and_migration_then_round_trips_meta() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let store = SqliteStore::open(&db).expect("open store");
        assert!(db.exists(), "database file should be created");

        {
            let conn = store.lock();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .expect("read journal mode");
            assert_eq!(mode, "wal");
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("read user_version");
            assert_eq!(version, SCHEMA_VERSION);
        }

        assert_eq!(store.meta_get("schema").expect("get absent"), None);
        store.meta_set("schema", "alpha").expect("set");
        assert_eq!(
            store.meta_get("schema").expect("get present"),
            Some("alpha".to_string())
        );
        // Upsert replaces rather than duplicating.
        store.meta_set("schema", "beta").expect("upsert");
        assert_eq!(
            store.meta_get("schema").expect("get updated"),
            Some("beta".to_string())
        );
    }

    #[test]
    fn reopening_an_existing_db_is_idempotent() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        SqliteStore::open(&db)
            .expect("first open")
            .meta_set("k", "v")
            .expect("write");
        let reopened = SqliteStore::open(&db).expect("second open");
        assert_eq!(reopened.meta_get("k").expect("read"), Some("v".to_string()));
    }
}
