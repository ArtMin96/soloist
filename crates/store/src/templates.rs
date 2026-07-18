//! The template repository — the core [`TemplateRepo`] port.
//!
//! One row per template in the `templates` table; `project_id` is NULL for the global scope and
//! `kind` separates a prompt from a scratchpad or todo starting shape. Every scope filter goes
//! through [`SCOPE_CLAUSE`] so the `(kind, NULL-vs-value)` matching lives in one place. Writes are
//! revision-guarded read-then-write under the single connection guard, with the kind-scope-name
//! unique index as the backstop (see the migration for why it is a COALESCE expression index, not a
//! UNIQUE constraint).

use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension};
use soloist_core::{
    ProjectId, StoreError, StoredTemplate, TemplateId, TemplateKind, TemplateRepo,
    TemplateWriteResult,
};

use crate::{sql_err, SqliteStore};

/// The columns every read selects, in order, so [`row_to_template`] decodes one shape.
const TEMPLATE_COLUMNS: &str = "id, kind, project_id, name, description, body, revision";

/// The scope filter, the same expression as the unique index: the kind, then the global scope (a
/// NULL `project_id`, which `=` can never match) mapped to the sentinel 0, which no project rowid
/// ever uses. `?1` is the kind, `?2` the scope value.
const SCOPE_CLAUSE: &str = "kind = ?1 AND COALESCE(project_id, 0) = COALESCE(?2, 0)";

/// The `?2` value [`SCOPE_CLAUSE`] compares against.
fn scope_param(project: Option<ProjectId>) -> Option<i64> {
    project.map(|id| id.get() as i64)
}

impl TemplateRepo for SqliteStore {
    fn write(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<TemplateWriteResult, StoreError> {
        let conn = self.lock();
        // Read the current revision and update-or-insert under one guard, so the guard check and
        // the write cannot interleave with a concurrent writer.
        let current = current_revision(&conn, kind, project, name)?;
        match (current, expected) {
            (Some(revision), Some(expected)) if revision == expected => {
                conn.execute(
                    &format!(
                        "UPDATE templates SET description = ?4, body = ?5, revision = ?6
                         WHERE {SCOPE_CLAUSE} AND name = ?3"
                    ),
                    (
                        kind.as_str(),
                        scope_param(project),
                        name,
                        description,
                        body,
                        (revision + 1) as i64,
                    ),
                )
                .map_err(sql_err)?;
                read_one(&conn, kind, project, name)?
                    .map(|stored| TemplateWriteResult::Written(Box::new(stored)))
                    .ok_or_else(|| StoreError::Backend("template vanished after write".into()))
            }
            (None, None) => {
                conn.execute(
                    "INSERT INTO templates (kind, project_id, name, description, body, revision)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                    (kind.as_str(), scope_param(project), name, description, body),
                )
                .map_err(sql_err)?;
                read_one(&conn, kind, project, name)?
                    .map(|stored| TemplateWriteResult::Written(Box::new(stored)))
                    .ok_or_else(|| StoreError::Backend("template vanished after create".into()))
            }
            (actual, _) => Ok(TemplateWriteResult::Conflict { actual }),
        }
    }

    fn read(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<StoredTemplate>, StoreError> {
        read_one(&self.lock(), kind, project, name)
    }

    fn list(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
    ) -> Result<Vec<StoredTemplate>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {TEMPLATE_COLUMNS} FROM templates WHERE {SCOPE_CLAUSE} ORDER BY name"
            ))
            .map_err(sql_err)?;
        let rows = stmt
            .query_map((kind.as_str(), scope_param(project)), row_to_template)
            .map_err(sql_err)?;
        let mut found = Vec::new();
        for row in rows {
            found.push(row.map_err(sql_err)?);
        }
        Ok(found)
    }

    fn delete(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                &format!("DELETE FROM templates WHERE {SCOPE_CLAUSE} AND name = ?3"),
                (kind.as_str(), scope_param(project), name),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }
}

fn read_one(
    conn: &Connection,
    kind: TemplateKind,
    project: Option<ProjectId>,
    name: &str,
) -> Result<Option<StoredTemplate>, StoreError> {
    conn.query_row(
        &format!("SELECT {TEMPLATE_COLUMNS} FROM templates WHERE {SCOPE_CLAUSE} AND name = ?3"),
        (kind.as_str(), scope_param(project), name),
        row_to_template,
    )
    .optional()
    .map_err(sql_err)
}

fn current_revision(
    conn: &Connection,
    kind: TemplateKind,
    project: Option<ProjectId>,
    name: &str,
) -> Result<Option<u64>, StoreError> {
    conn.query_row(
        &format!("SELECT revision FROM templates WHERE {SCOPE_CLAUSE} AND name = ?3"),
        (kind.as_str(), scope_param(project), name),
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(sql_err)
    .map(|revision| revision.map(|value| value as u64))
}

fn row_to_template(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredTemplate> {
    let kind_str: String = row.get(1)?;
    let kind = TemplateKind::from_db(&kind_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            Type::Text,
            format!("unknown template kind {kind_str:?}").into(),
        )
    })?;
    Ok(StoredTemplate {
        id: TemplateId::from_raw(row.get::<_, i64>(0)? as u64),
        kind,
        project: row
            .get::<_, Option<i64>>(2)?
            .map(|id| ProjectId::from_raw(id as u64)),
        name: row.get(3)?,
        description: row.get(4)?,
        body: row.get(5)?,
        revision: row.get::<_, i64>(6)? as u64,
    })
}

#[cfg(test)]
#[path = "templates_tests.rs"]
mod tests;
