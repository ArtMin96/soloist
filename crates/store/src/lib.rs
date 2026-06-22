//! SQLite-backed implementation of the core's durable ports.
//!
//! One connection (WAL, foreign keys on) behind a `Mutex` backs the durable
//! repositories, following the repository pattern: each port lives in its own
//! module (`meta`, `projects`, `trust`, `agents`) implementing the matching core trait.
//! Schema changes are versioned, idempotent [`migrate`]ions. SQLite is bundled, so the
//! binary carries its own engine and needs no system `libsqlite3`.

mod agents;
mod meta;
mod migrate;
mod projects;
mod runtime;
mod trust;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use soloist_core::StoreError;

pub use runtime::FileRuntimeState;

/// A durable store backed by a single SQLite connection.
///
/// `rusqlite::Connection` is `Send` but not `Sync`, so it is guarded by a `Mutex`
/// to satisfy the `Send + Sync` repository contracts. Critical sections are tiny
/// (single-statement reads/writes) and never held across an `await`.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Opens (creating if needed) the database at `path` in WAL mode with foreign
    /// keys enabled, running pending migrations. Parent directories are created.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let conn = Connection::open(path).map_err(sql_err)?;
        // WAL keeps readers from blocking the writer over a long-running session.
        let _mode: String = conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .map_err(sql_err)?;
        configure(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Opens the database in the app data directory, creating that directory (restricted
    /// to its owner) through the one [`soloist_ipc::ensure_data_dir`] resolution the IPC
    /// socket also uses, so the database and the socket always share one owner-only home.
    pub fn open_default() -> Result<Self, StoreError> {
        let dir = soloist_ipc::ensure_data_dir().map_err(io_err)?;
        Self::open(&dir.join("soloist.db"))
    }

    /// Opens an ephemeral in-memory database (migrated, foreign keys on, but not
    /// durable). Used as a graceful fallback when the durable location is
    /// unavailable, so the app stays usable rather than failing to launch.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(sql_err)?;
        configure(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Locks the connection, recovering the guard if a previous holder panicked.
    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        match self.conn.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// Enables foreign-key enforcement (so trust cascades when a project is removed) and
/// runs migrations. Shared by the durable and in-memory constructors.
fn configure(conn: &Connection) -> Result<(), StoreError> {
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(sql_err)?;
    migrate::migrate(conn)
}

pub(crate) fn sql_err(err: rusqlite::Error) -> StoreError {
    StoreError::Backend(err.to_string())
}

pub(crate) fn io_err(err: std::io::Error) -> StoreError {
    StoreError::Backend(err.to_string())
}

/// SQLite stores text as UTF-8, so a path that is not valid UTF-8 cannot be used as
/// a key. In practice project roots and icon paths are UTF-8; non-UTF-8 is rejected
/// loudly rather than corrupted by a lossy conversion.
pub(crate) fn path_str(path: &Path) -> Result<&str, StoreError> {
    path.to_str()
        .ok_or_else(|| StoreError::Backend(format!("path is not valid UTF-8: {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn open_enables_wal_and_migrates_to_the_current_version() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let store = SqliteStore::open(&db).expect("open store");
        assert!(db.exists(), "database file should be created");

        let conn = store.lock();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("read journal mode");
        assert_eq!(mode, "wal");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version");
        assert_eq!(version, migrate::SCHEMA_VERSION);
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("read foreign_keys");
        assert_eq!(fk, 1, "foreign keys must be enforced for trust cascade");
    }
}
