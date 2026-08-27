//! Shared row-level mechanics behind the coordination document tables — scratchpads and diagrams
//! are both a `(project_id, name)`-addressed table with a revision-guarded body, a JSON tag set,
//! an archived flag, and a durable id, and every state-dependent operation holds the connection
//! guard the caller already took for its whole span. Each table keeps its own full SQL statements
//! as static literals ([`DocStatements`]) and its own row decoder, so every query stays one
//! greppable literal at its call site rather than text built at runtime; this module holds only
//! the guard, conflict, decode, and tag read-modify-write logic that runs those statements
//! identically for both tables.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, Row};
use soloist_core::{ProjectId, StoreError};

use crate::sql_err;

/// One table's full set of statements, each a static literal naming that table and its columns —
/// composed here, never built from interpolated identifiers, so every query stays a single
/// greppable piece of text at its own call site.
pub(crate) struct DocStatements {
    /// The noun a "row vanished" error and a tag (de)serialize error name, e.g. `"scratchpad"`.
    pub(crate) label: &'static str,
    pub(crate) select_revision: &'static str,
    pub(crate) select_one: &'static str,
    pub(crate) select_list: &'static str,
    pub(crate) select_tags: &'static str,
    pub(crate) select_contains: &'static str,
    pub(crate) insert: &'static str,
    pub(crate) update_body: &'static str,
    pub(crate) update_name: &'static str,
    pub(crate) update_tags: &'static str,
    pub(crate) update_archived: &'static str,
    pub(crate) delete: &'static str,
}

/// Decodes one row into `T`. The outer `rusqlite::Result` carries a column error; the inner
/// [`StoreError`] carries a JSON deserialize failure, kept distinct so neither is mistaken for the
/// other.
pub(crate) type RowDecoder<T> = fn(&Row<'_>) -> rusqlite::Result<Result<T, StoreError>>;

/// The outcome of a revision-guarded [`DocTable::write`], generic over the stored row type: either
/// the write applied and the row at its new revision, or the expected revision no longer matched
/// and nothing changed. Each table's repository impl maps this into its own core result enum.
pub(crate) enum DocWrite<T> {
    Written(Box<T>),
    Conflict { actual: Option<u64> },
}

/// The outcome of a [`DocTable::rename`], generic over the stored row type.
pub(crate) enum DocRename<T> {
    Renamed(Box<T>),
    NotFound,
    NameTaken,
}

/// Gives [`DocTable::update_tags`] access to a stored row's tag set without it knowing the
/// concrete row type.
pub(crate) trait TaggedRow {
    fn tags_mut(&mut self) -> &mut Vec<String>;
}

/// One document table, addressed by its static [`DocStatements`] and row decoder. Cheap to build —
/// two references and a function pointer — so callers construct it fresh per operation rather than
/// storing it.
pub(crate) struct DocTable<'a, T> {
    statements: &'a DocStatements,
    decode: RowDecoder<T>,
}

impl<'a, T> DocTable<'a, T> {
    pub(crate) fn new(statements: &'a DocStatements, decode: RowDecoder<T>) -> Self {
        Self { statements, decode }
    }

    /// The current revision of `(project, name)` over an already-held guard, or `None` if absent.
    pub(crate) fn current_revision(
        &self,
        conn: &Connection,
        project: ProjectId,
        name: &str,
    ) -> Result<Option<u64>, StoreError> {
        conn.query_row(
            self.statements.select_revision,
            (project.get() as i64, name),
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sql_err)
        .map(|revision| revision.map(|revision| revision as u64))
    }

    /// One row by `(project, name)` over an already-held guard, or `None` if absent.
    pub(crate) fn read_one(
        &self,
        conn: &Connection,
        project: ProjectId,
        name: &str,
    ) -> Result<Option<T>, StoreError> {
        conn.query_row(
            self.statements.select_one,
            (project.get() as i64, name),
            self.decode,
        )
        .optional()
        .map_err(sql_err)?
        .transpose()
    }

    /// Every row in `project`, ordered by name.
    pub(crate) fn list(&self, conn: &Connection, project: ProjectId) -> Result<Vec<T>, StoreError> {
        let mut stmt = conn.prepare(self.statements.select_list).map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], self.decode)
            .map_err(sql_err)?;
        let mut found = Vec::new();
        for row in rows {
            found.push(row.map_err(sql_err)??);
        }
        Ok(found)
    }

    /// Whether `project` owns the row `id`.
    pub(crate) fn contains(
        &self,
        conn: &Connection,
        project: ProjectId,
        id: u64,
    ) -> Result<bool, StoreError> {
        conn.query_row(
            self.statements.select_contains,
            (project.get() as i64, id as i64),
            |_| Ok(()),
        )
        .optional()
        .map(|found| found.is_some())
        .map_err(sql_err)
    }

    /// Creates or updates `(project, name)` with `body`, revision-guarded in one atomic step over
    /// an already-held guard: `expected` is `None` to create (applies only if absent) or the
    /// current revision to update (applies only if it still matches). `now` stamps the row's
    /// recency column on an applied write.
    pub(crate) fn write(
        &self,
        conn: &Connection,
        project: ProjectId,
        name: &str,
        body: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<DocWrite<T>, StoreError> {
        // Read the current revision and update-or-insert under the caller's guard, so the guard
        // check and the write cannot interleave with a concurrent writer.
        let current = self.current_revision(conn, project, name)?;
        match (current, expected) {
            // Update the existing row at the expected revision, bumping it and stamping the write.
            (Some(revision), Some(expected)) if revision == expected => {
                conn.execute(
                    self.statements.update_body,
                    (
                        project.get() as i64,
                        name,
                        body,
                        (revision + 1) as i64,
                        now as i64,
                    ),
                )
                .map_err(sql_err)?;
                self.read_one(conn, project, name)?
                    .map(|stored| DocWrite::Written(Box::new(stored)))
                    .ok_or_else(|| vanished(self.statements.label, "write"))
            }
            // Create a fresh row only when none exists and the caller expected absence.
            (None, None) => {
                conn.execute(
                    self.statements.insert,
                    (project.get() as i64, name, body, now as i64),
                )
                .map_err(sql_err)?;
                self.read_one(conn, project, name)?
                    .map(|stored| DocWrite::Written(Box::new(stored)))
                    .ok_or_else(|| vanished(self.statements.label, "create"))
            }
            // Any other pairing is a revision mismatch; nothing is written.
            (actual, _) => Ok(DocWrite::Conflict { actual }),
        }
    }

    /// Renames `(project, from)` to `to` over an already-held guard, checking target uniqueness as
    /// part of the update so two renames cannot both take one name.
    pub(crate) fn rename(
        &self,
        conn: &Connection,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<DocRename<T>, StoreError> {
        // Reject a taken target before the update (a clearer outcome than the UNIQUE violation),
        // and do both under one guard so two renames cannot both take one name.
        if from != to && self.current_revision(conn, project, to)?.is_some() {
            return Ok(DocRename::NameTaken);
        }
        let updated = conn
            .execute(
                self.statements.update_name,
                (project.get() as i64, from, to),
            )
            .map_err(sql_err)?;
        if updated == 0 {
            return Ok(DocRename::NotFound);
        }
        self.read_one(conn, project, to)?
            .map(|stored| DocRename::Renamed(Box::new(stored)))
            .ok_or_else(|| vanished(self.statements.label, "rename"))
    }

    /// The distinct tags used across `project`'s rows, sorted.
    pub(crate) fn tags(
        &self,
        conn: &Connection,
        project: ProjectId,
    ) -> Result<Vec<String>, StoreError> {
        let mut stmt = conn.prepare(self.statements.select_tags).map_err(sql_err)?;
        let rows = stmt
            .query_map([project.get() as i64], |row| row.get::<_, String>(0))
            .map_err(sql_err)?;
        let mut distinct = BTreeSet::new();
        for row in rows {
            for tag in decode_tags(&row.map_err(sql_err)?, self.statements.label)? {
                distinct.insert(tag);
            }
        }
        Ok(distinct.into_iter().collect())
    }

    /// Sets the archived flag of `(project, name)` over an already-held guard, returning the
    /// updated row, or `None` if absent.
    pub(crate) fn set_archived(
        &self,
        conn: &Connection,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<T>, StoreError> {
        let updated = conn
            .execute(
                self.statements.update_archived,
                (project.get() as i64, name, archived as i64),
            )
            .map_err(sql_err)?;
        if updated == 0 {
            return Ok(None);
        }
        self.read_one(conn, project, name)
    }

    /// Deletes `(project, name)` over an already-held guard, returning whether one was removed.
    pub(crate) fn delete(
        &self,
        conn: &Connection,
        project: ProjectId,
        name: &str,
    ) -> Result<bool, StoreError> {
        conn.execute(self.statements.delete, (project.get() as i64, name))
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }

    /// Reads `(project, name)`'s tag set, applies `change`, writes it back, and returns the
    /// updated row — all over an already-held guard, so a concurrent tag change is not lost.
    /// `None` if the row does not exist. The tag set is stored sorted, normalized here after every
    /// change so add and remove leave the same canonical order.
    pub(crate) fn update_tags(
        &self,
        conn: &Connection,
        project: ProjectId,
        name: &str,
        change: impl FnOnce(&mut Vec<String>),
    ) -> Result<Option<T>, StoreError>
    where
        T: TaggedRow,
    {
        let Some(mut stored) = self.read_one(conn, project, name)? else {
            return Ok(None);
        };
        change(stored.tags_mut());
        stored.tags_mut().sort();
        let tags_json = serialize_tags(stored.tags_mut(), self.statements.label)?;
        conn.execute(
            self.statements.update_tags,
            (project.get() as i64, name, &tags_json),
        )
        .map_err(sql_err)?;
        Ok(Some(stored))
    }
}

/// The error a write/rename/transfer reports when the row it just wrote does not read back — a
/// bug, never an expected outcome, named for the caller's noun (`label`) and the step it happened
/// at.
pub(crate) fn vanished(label: &str, when: &str) -> StoreError {
    StoreError::Backend(format!("{label} vanished after {when}"))
}

/// Serializes a tag set to the JSON array a table's `tags` column stores.
pub(crate) fn serialize_tags(tags: &[String], label: &str) -> Result<String, StoreError> {
    serde_json::to_string(tags)
        .map_err(|err| StoreError::Backend(format!("serialize {label} tags: {err}")))
}

/// Deserializes a table's `tags` column JSON array into a tag set.
pub(crate) fn decode_tags(json: &str, label: &str) -> Result<Vec<String>, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize {label} tags: {err}")))
}
