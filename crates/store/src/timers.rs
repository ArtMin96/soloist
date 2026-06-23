//! The coordination timer repository — the core [`TimerRepo`] port.
//!
//! One row per timer in the `timers` table, keyed by the `AUTOINCREMENT` id the store assigns
//! (never reused, so a cancelled timer's id can never name a later one). The fire *condition*
//! (`FireCond`) is stored as its JSON, so the persisted shape is exactly the domain type and cannot
//! drift; the absolute `deadline_unix_millis` is a column of its own so pausing can freeze it and
//! resuming can re-arm it with one statement. A `paused` flag carries the [`TimerStatus`], and
//! `remaining_millis` holds the time left while paused. Every state-dependent method holds the
//! single connection guard for its whole operation, so the scheduler claiming a timer to fire it
//! cannot interleave with its owner pausing or cancelling it. The `project_id` foreign key
//! cascades, so removing a project drops its timers.

use rusqlite::{Connection, OptionalExtension, Row};
use soloist_core::{
    FireCond, NewTimer, ProcessId, ProjectId, StoreError, StoredTimer, TimerId, TimerRepo,
    TimerStatus,
};

use crate::{sql_err, SqliteStore};

/// The columns every read selects, in order, so [`row_to_timer`] decodes one shape.
const TIMER_COLUMNS: &str =
    "id, project_id, owner, body, fire, deadline_unix_millis, paused, remaining_millis";

impl TimerRepo for SqliteStore {
    fn create(&self, timer: &NewTimer) -> Result<TimerId, StoreError> {
        let fire = serialize_fire(&timer.fire)?;
        let conn = self.lock();
        conn.execute(
            "INSERT INTO timers
                 (project_id, owner, body, fire, deadline_unix_millis, paused, remaining_millis)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, NULL)",
            (
                timer.project.get() as i64,
                timer.owner.get() as i64,
                &timer.body,
                &fire,
                timer.deadline_unix_millis as i64,
            ),
        )
        .map_err(sql_err)?;
        Ok(TimerId::from_raw(conn.last_insert_rowid() as u64))
    }

    fn armed(&self) -> Result<Vec<StoredTimer>, StoreError> {
        let conn = self.lock();
        collect_timers(
            &conn,
            &format!("SELECT {TIMER_COLUMNS} FROM timers WHERE paused = 0"),
            [],
        )
    }

    fn take_if_armed(&self, id: TimerId) -> Result<Option<StoredTimer>, StoreError> {
        let conn = self.lock();
        // Read the armed row and remove it under one guard, so a concurrent pause or cancel either
        // wins the race (the row is no longer armed/present and this returns `None`) or loses (the
        // claim removes it first). A paused row is never claimed.
        let claimed = conn
            .query_row(
                &format!("SELECT {TIMER_COLUMNS} FROM timers WHERE id = ?1 AND paused = 0"),
                [id.get() as i64],
                row_to_timer,
            )
            .optional()
            .map_err(sql_err)?
            .transpose()?;
        if claimed.is_some() {
            conn.execute("DELETE FROM timers WHERE id = ?1", [id.get() as i64])
                .map_err(sql_err)?;
        }
        Ok(claimed)
    }

    fn cancel(&self, id: TimerId, owner: ProcessId) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                "DELETE FROM timers WHERE id = ?1 AND owner = ?2",
                (id.get() as i64, owner.get() as i64),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn pause(&self, id: TimerId, owner: ProcessId, now: u64) -> Result<bool, StoreError> {
        // Freeze the time that remains (never negative) and mark it paused, in one conditional
        // statement: an armed row owned by the caller becomes paused; anything else is untouched.
        self.lock()
            .execute(
                "UPDATE timers
                     SET paused = 1,
                         remaining_millis = MAX(0, deadline_unix_millis - ?3)
                 WHERE id = ?1 AND owner = ?2 AND paused = 0",
                (id.get() as i64, owner.get() as i64, now as i64),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn resume(&self, id: TimerId, owner: ProcessId, now: u64) -> Result<bool, StoreError> {
        // Re-arm with a deadline of now plus the time that remained, in one conditional statement.
        self.lock()
            .execute(
                "UPDATE timers
                     SET paused = 0,
                         deadline_unix_millis = ?3 + COALESCE(remaining_millis, 0),
                         remaining_millis = NULL
                 WHERE id = ?1 AND owner = ?2 AND paused = 1",
                (id.get() as i64, owner.get() as i64, now as i64),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    fn list(&self, owner: ProcessId) -> Result<Vec<StoredTimer>, StoreError> {
        let conn = self.lock();
        collect_timers(
            &conn,
            &format!("SELECT {TIMER_COLUMNS} FROM timers WHERE owner = ?1 ORDER BY id"),
            [owner.get() as i64],
        )
    }

    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError> {
        self.lock()
            .execute("DELETE FROM timers WHERE owner = ?1", [owner.get() as i64])
            .map_err(sql_err)
    }

    fn clear(&self) -> Result<usize, StoreError> {
        self.lock()
            .execute("DELETE FROM timers", [])
            .map_err(sql_err)
    }
}

/// Serializes a [`FireCond`] to the JSON the `fire` column stores.
fn serialize_fire(fire: &FireCond) -> Result<String, StoreError> {
    serde_json::to_string(fire)
        .map_err(|err| StoreError::Backend(format!("serialize timer fire condition: {err}")))
}

/// Runs `sql` (selecting [`TIMER_COLUMNS`]) and decodes every row, surfacing a malformed `fire`
/// JSON as a backend error rather than silently dropping the timer.
fn collect_timers(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<StoredTimer>, StoreError> {
    let mut stmt = conn.prepare(sql).map_err(sql_err)?;
    let rows = stmt.query_map(params, row_to_timer).map_err(sql_err)?;
    let mut timers = Vec::new();
    for row in rows {
        timers.push(row.map_err(sql_err)??);
    }
    Ok(timers)
}

/// Decodes one row into a [`StoredTimer`]. The outer `rusqlite::Result` carries a column error; the
/// inner [`StoreError`] carries a `fire`-JSON deserialize failure, kept distinct so neither is
/// mistaken for the other.
fn row_to_timer(row: &Row<'_>) -> rusqlite::Result<Result<StoredTimer, StoreError>> {
    let id: i64 = row.get(0)?;
    let project: i64 = row.get(1)?;
    let owner: i64 = row.get(2)?;
    let body: String = row.get(3)?;
    let fire_json: String = row.get(4)?;
    let deadline: i64 = row.get(5)?;
    let paused: i64 = row.get(6)?;
    let remaining: Option<i64> = row.get(7)?;
    Ok(decode_fire(&fire_json).map(|fire| StoredTimer {
        id: TimerId::from_raw(id as u64),
        project: ProjectId::from_raw(project as u64),
        owner: ProcessId::from_raw(owner as u64),
        body,
        fire,
        deadline_unix_millis: deadline as u64,
        status: if paused != 0 {
            TimerStatus::Paused
        } else {
            TimerStatus::Armed
        },
        remaining_on_pause_millis: remaining.map(|millis| millis as u64),
    }))
}

/// Deserializes the `fire` column's JSON into a [`FireCond`].
fn decode_fire(json: &str) -> Result<FireCond, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize timer fire condition: {err}")))
}

#[cfg(test)]
#[path = "timers_tests.rs"]
mod tests;
