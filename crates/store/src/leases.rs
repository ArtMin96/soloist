//! The coordination lease repository — the core [`LockRepo`] port.
//!
//! One row per `(project_id, key)` in the `leases` table. This is dumb persistence: the TTL
//! expiry policy lives in the core lease aggregate (which compares the stored `expires_unix_millis`
//! against the wall clock), so [`get`](LockRepo::get) returns whatever is stored regardless of
//! expiry. The `project_id` foreign key cascades, so removing a project drops its leases.

use rusqlite::OptionalExtension;
use soloist_core::{LockRepo, ProcessId, ProjectId, StoreError, StoredLease};

use crate::{sql_err, SqliteStore};

impl LockRepo for SqliteStore {
    fn get(&self, project: ProjectId, key: &str) -> Result<Option<StoredLease>, StoreError> {
        self.lock()
            .query_row(
                "SELECT owner, acquired_unix_millis, expires_unix_millis
                 FROM leases WHERE project_id = ?1 AND key = ?2",
                (project.get() as i64, key),
                |row| {
                    let owner: i64 = row.get(0)?;
                    let acquired: i64 = row.get(1)?;
                    let expires: i64 = row.get(2)?;
                    Ok(StoredLease {
                        project,
                        key: key.to_owned(),
                        owner: ProcessId::from_raw(owner as u64),
                        acquired_unix_millis: acquired as u64,
                        expires_unix_millis: expires as u64,
                    })
                },
            )
            .optional()
            .map_err(sql_err)
    }

    fn put(&self, lease: &StoredLease) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "INSERT OR REPLACE INTO leases
                     (project_id, key, owner, acquired_unix_millis, expires_unix_millis)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    lease.project.get() as i64,
                    &lease.key,
                    lease.owner.get() as i64,
                    lease.acquired_unix_millis as i64,
                    lease.expires_unix_millis as i64,
                ),
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn remove(&self, project: ProjectId, key: &str) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                "DELETE FROM leases WHERE project_id = ?1 AND key = ?2",
                (project.get() as i64, key),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError> {
        self.lock()
            .execute("DELETE FROM leases WHERE owner = ?1", [owner.get() as i64])
            .map_err(sql_err)
    }

    fn clear(&self) -> Result<usize, StoreError> {
        self.lock()
            .execute("DELETE FROM leases", [])
            .map_err(sql_err)
    }
}

#[cfg(test)]
#[path = "leases_tests.rs"]
mod tests;
