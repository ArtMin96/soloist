//! The v17 rebuild of `projects` onto an `AUTOINCREMENT` primary key.
//!
//! Lives apart from the migration ledger because it is a procedure rather than a step: SQLite
//! cannot add `AUTOINCREMENT` in place, so the table is rebuilt, and the sequencing that makes that
//! safe (pragma, transaction, orphan check) is the whole content of this module.

use rusqlite::{Connection, OptionalExtension};
use soloist_core::StoreError;

use crate::sql_err;

/// Rebuilds `projects` with an `AUTOINCREMENT` primary key, preserving every row's id so no
/// foreign key that references it is orphaned.
///
/// Follows SQLite's documented table-rebuild procedure: foreign keys are disabled first (so the
/// `DROP` does not cascade every project-scoped row away, and so the `RENAME` does not rewrite the
/// child tables' `REFERENCES projects` clauses to the temporary name), the swap runs in one
/// transaction, and `foreign_key_check` verifies nothing was orphaned before it commits. The
/// pragma is restored whether the swap succeeded or failed — which is only meaningful because the
/// swap rolls back before returning, since a pragma inside an open transaction is ignored. Copying
/// the ids also seeds `sqlite_sequence` to the current maximum, so the high-water mark carries over
/// rather than restarting.
pub(crate) fn rebuild_projects_with_autoincrement(conn: &Connection) -> Result<(), StoreError> {
    if projects_id_autoincrements(conn)? {
        return Ok(());
    }
    let foreign_keys: bool = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(sql_err)?;
    // A pragma is a no-op inside a transaction, so this must precede the `BEGIN`.
    conn.pragma_update(None, "foreign_keys", false)
        .map_err(sql_err)?;
    let rebuilt = swap_in_rebuilt_projects(conn);
    conn.pragma_update(None, "foreign_keys", foreign_keys)
        .map_err(sql_err)?;
    rebuilt
}

/// The transactional half of the rebuild: build the replacement, copy the rows, swap the names,
/// and commit only once the foreign keys still resolve.
///
/// Every failure after the `BEGIN` rolls back here, so this returns with no transaction of ours
/// left open and the next run sees the original table. That matters beyond tidiness: the caller
/// restores `foreign_keys` afterwards, and a pragma is silently ignored inside an open transaction,
/// so an un-rolled-back failure would leave foreign keys disabled on a live connection.
fn swap_in_rebuilt_projects(conn: &Connection) -> Result<(), StoreError> {
    match rebuilt_projects_committed(conn) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Best-effort: if the `BEGIN` itself never took, there is nothing to roll back and the
            // failure to do so is not the error worth reporting.
            let _ = conn.execute_batch("ROLLBACK;");
            Err(err)
        }
    }
}

/// The swap itself, from `BEGIN` to `COMMIT`. Every exit is an error the caller rolls back.
///
/// The column list is deliberately a copy of `projects` as it stood at v16, not a shared definition
/// with the `CREATE TABLE` in the v2 step. A migration step describes the schema at one moment in
/// history and must keep describing it: a later column arrives through its own step, on a database
/// this one has long since rebuilt. Sharing one definition between the two would silently rewrite
/// what an old database is upgraded *through*, which is the one thing a migration may never do.
fn rebuilt_projects_committed(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "BEGIN;
         DROP TABLE IF EXISTS projects_rebuilt;
         CREATE TABLE projects_rebuilt (
             id   INTEGER PRIMARY KEY AUTOINCREMENT,
             root TEXT NOT NULL UNIQUE,
             name TEXT,
             icon TEXT
         );
         INSERT INTO projects_rebuilt (id, root, name, icon)
             SELECT id, root, name, icon FROM projects;
         DROP TABLE projects;
         ALTER TABLE projects_rebuilt RENAME TO projects;",
    )
    .map_err(sql_err)?;
    match orphaned_rows(conn)? {
        0 => conn.execute_batch("COMMIT;").map_err(sql_err),
        orphans => Err(StoreError::Backend(format!(
            "rebuilding projects would orphan {orphans} referencing row(s)"
        ))),
    }
}

/// How many rows in the database reference a row that is not there.
fn orphaned_rows(conn: &Connection) -> Result<i64, StoreError> {
    conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
        row.get(0)
    })
    .map_err(sql_err)
}

/// Whether `projects` already declares an `AUTOINCREMENT` id — the guard that keeps the rebuild a
/// no-op on a re-run, like the other guarded steps. The declaration is only recoverable from the
/// stored `CREATE TABLE` text; no pragma reports it.
fn projects_id_autoincrements(conn: &Connection) -> Result<bool, StoreError> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'projects'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_err)?;
    Ok(sql.is_some_and(|sql| sql.to_uppercase().contains("AUTOINCREMENT")))
}

#[cfg(test)]
#[path = "projects_rebuild_tests.rs"]
mod tests;
