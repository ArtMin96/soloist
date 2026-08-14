//! The trust repository — the core [`TrustRepo`] port.
//!
//! Trust rows in `trust` are keyed by `(project_id, variant_hash)`: the presence of a row means
//! that exact command variant is trusted within that project. Rows in `project_trust` are keyed
//! by the project alone: the presence of one means the user has authorised Soloist to make
//! changes within that project. Both `project_id` foreign keys cascade, so removing a project
//! drops everything it was trusted for with it.

use rusqlite::OptionalExtension;
use soloist_core::{Hash, ProcessId, ProjectId, StoreError, TrustGrant, TrustRepo};

use crate::{sql_err, SqliteStore};

impl TrustRepo for SqliteStore {
    fn is_trusted(&self, project: ProjectId, variant: &Hash) -> Result<bool, StoreError> {
        let found: Option<i64> = self
            .lock()
            .query_row(
                "SELECT 1 FROM trust WHERE project_id = ?1 AND variant_hash = ?2",
                (project.get() as i64, variant.to_hex()),
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_err)?;
        Ok(found.is_some())
    }

    fn set_trusted(
        &self,
        project: ProjectId,
        variant: &Hash,
        command: &str,
    ) -> Result<(), StoreError> {
        // The command line is refreshed on every grant while the provenance columns are left
        // alone, so re-trusting a variant the user authored does not erase the record of an
        // earlier request for it, and a row written before the column existed gains its command.
        self.lock()
            .execute(
                "INSERT INTO trust (project_id, variant_hash, command) VALUES (?1, ?2, ?3)
                 ON CONFLICT (project_id, variant_hash) DO UPDATE SET command = excluded.command",
                (project.get() as i64, variant.to_hex(), command),
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn set_trusted_with_provenance(
        &self,
        project: ProjectId,
        variant: &Hash,
        command: &str,
        requested_by: ProcessId,
        reason: &str,
        granted_at_unix_millis: u64,
    ) -> Result<(), StoreError> {
        // Upsert rather than insert-or-ignore: a variant the user had already trusted without
        // provenance now has a record of who asked and why, which is what the review surface needs
        // to show. The grant itself is unchanged either way — the row's existence is the trust.
        self.lock()
            .execute(
                "INSERT INTO trust
                     (project_id, variant_hash, command, requested_by, reason, granted_at_unix_millis)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (project_id, variant_hash) DO UPDATE SET
                     command = excluded.command,
                     requested_by = excluded.requested_by,
                     reason = excluded.reason,
                     granted_at_unix_millis = excluded.granted_at_unix_millis",
                (
                    project.get() as i64,
                    variant.to_hex(),
                    command,
                    requested_by.get() as i64,
                    reason,
                    granted_at_unix_millis as i64,
                ),
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn list_grants(&self, project: ProjectId) -> Result<Vec<TrustGrant>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT variant_hash, command, requested_by, reason, granted_at_unix_millis
                 FROM trust WHERE project_id = ?1 ORDER BY granted_at_unix_millis DESC, variant_hash",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], |row| {
                Ok(TrustGrant {
                    variant_hash: row.get(0)?,
                    command: row.get(1)?,
                    requested_by: row
                        .get::<_, Option<i64>>(2)?
                        .map(|raw| ProcessId::from_raw(raw as u64)),
                    reason: row.get(3)?,
                    granted_at_unix_millis: row.get::<_, Option<i64>>(4)?.map(|raw| raw as u64),
                })
            })
            .map_err(sql_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql_err)
    }

    fn revoke(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "DELETE FROM trust WHERE project_id = ?1 AND variant_hash = ?2",
                (project.get() as i64, variant.to_hex()),
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn is_project_trusted(&self, project: ProjectId) -> Result<bool, StoreError> {
        let found: Option<i64> = self
            .lock()
            .query_row(
                "SELECT 1 FROM project_trust WHERE project_id = ?1",
                [project.get() as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_err)?;
        Ok(found.is_some())
    }

    fn set_project_trusted(&self, project: ProjectId) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "INSERT OR IGNORE INTO project_trust (project_id) VALUES (?1)",
                [project.get() as i64],
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn revoke_project(&self, project: ProjectId) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "DELETE FROM project_trust WHERE project_id = ?1",
                [project.get() as i64],
            )
            .map(|_| ())
            .map_err(sql_err)
    }
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
