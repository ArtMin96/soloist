//! The coordination lease repository — the core [`LockRepo`] port.
//!
//! One row per `(project_id, key)` in the `leases` table. Each method holds the single connection
//! guard for its whole operation, so a check-and-write is atomic: [`acquire`](LockRepo::acquire)
//! grants only when the slot is free, expired, or already the caller's, in one conditional upsert,
//! so two racing acquires cannot both win. The TTL *policy* (default, clamp) lives in the core
//! lease aggregate; the store enforces the one expiry rule a lease is live iff its
//! `expires_unix_millis` is strictly after `now`. The `project_id` foreign key cascades, so
//! removing a project drops its leases.

use rusqlite::OptionalExtension;
use soloist_core::{LockRepo, ProcessId, ProjectId, StoreError, StoredLease};

use crate::{sql_err, SqliteStore};

impl LockRepo for SqliteStore {
    fn acquire(
        &self,
        candidate: &StoredLease,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        let conn = self.lock();
        // One statement: insert when free, or overwrite on conflict only when the existing row
        // has expired at `now` or is already the caller's (a renewal). When neither holds, the
        // ON CONFLICT update is suppressed and zero rows change — the caller lost the race.
        let granted = conn
            .execute(
                "INSERT INTO leases
                     (project_id, key, owner, acquired_unix_millis, expires_unix_millis)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(project_id, key) DO UPDATE SET
                     owner = excluded.owner,
                     acquired_unix_millis = excluded.acquired_unix_millis,
                     expires_unix_millis = excluded.expires_unix_millis
                 WHERE leases.expires_unix_millis <= ?6 OR leases.owner = ?3",
                (
                    candidate.project.get() as i64,
                    &candidate.key,
                    candidate.owner.get() as i64,
                    candidate.acquired_unix_millis as i64,
                    candidate.expires_unix_millis as i64,
                    now as i64,
                ),
            )
            .map_err(sql_err)?;
        if granted > 0 {
            return Ok(None);
        }
        // Lost: report the live holder that blocked the grant. Still under the same guard, so the
        // row cannot have changed since the conflicting upsert above.
        self.live_locked(&conn, candidate.project, &candidate.key, now)
    }

    fn live(
        &self,
        project: ProjectId,
        key: &str,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        let conn = self.lock();
        // Prune the row only if it has expired (so a fresh concurrent acquire is never dropped),
        // then read whatever live lease remains — both under one guard.
        conn.execute(
            "DELETE FROM leases
             WHERE project_id = ?1 AND key = ?2 AND expires_unix_millis <= ?3",
            (project.get() as i64, key, now as i64),
        )
        .map_err(sql_err)?;
        self.live_locked(&conn, project, key, now)
    }

    fn release(&self, project: ProjectId, key: &str, owner: ProcessId) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                "DELETE FROM leases WHERE project_id = ?1 AND key = ?2 AND owner = ?3",
                (project.get() as i64, key, owner.get() as i64),
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

impl SqliteStore {
    /// The live lease for `(project, key)` at `now` over an already-held connection guard, or
    /// `None` if free or expired. Shared by [`acquire`](LockRepo::acquire)'s holder report and
    /// [`live`](LockRepo::live)'s read so the expiry rule is applied in one place.
    fn live_locked(
        &self,
        conn: &rusqlite::Connection,
        project: ProjectId,
        key: &str,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        conn.query_row(
            "SELECT owner, acquired_unix_millis, expires_unix_millis
             FROM leases
             WHERE project_id = ?1 AND key = ?2 AND expires_unix_millis > ?3",
            (project.get() as i64, key, now as i64),
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
}

#[cfg(test)]
#[path = "leases_tests.rs"]
mod tests;
