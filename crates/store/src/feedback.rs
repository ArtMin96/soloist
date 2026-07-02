//! The feedback repository — the core [`FeedbackRepo`] port.
//!
//! One row per submitted message in the `feedback` table, append-only and global (no
//! project foreign key). Nothing is ever transmitted; the table exists so the user can read
//! feedback back locally.

use soloist_core::{FeedbackEntry, FeedbackRepo, StoreError};

use crate::{sql_err, SqliteStore};

impl FeedbackRepo for SqliteStore {
    fn append(
        &self,
        message: &str,
        submitted_unix_millis: u64,
    ) -> Result<FeedbackEntry, StoreError> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO feedback (message, submitted_unix_millis) VALUES (?1, ?2)",
            (message, submitted_unix_millis as i64),
        )
        .map_err(sql_err)?;
        Ok(FeedbackEntry {
            id: conn.last_insert_rowid() as u64,
            message: message.to_owned(),
            submitted_unix_millis,
        })
    }

    fn list(&self) -> Result<Vec<FeedbackEntry>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, message, submitted_unix_millis FROM feedback ORDER BY id")
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FeedbackEntry {
                    id: row.get::<_, i64>(0)? as u64,
                    message: row.get(1)?,
                    submitted_unix_millis: row.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(sql_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql_err)
    }
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
