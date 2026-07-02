//! The prompt-template repository — the core [`PromptTemplateRepo`] port.
//!
//! One row per template in the `prompt_templates` table; `project_id` is NULL for the
//! global scope. Every scope filter goes through [`SCOPE_CLAUSE`] so NULL-vs-value matching
//! lives in one place. Writes are revision-guarded read-then-write under the single
//! connection guard, with the scope-name unique index as the backstop (see the migration
//! for why it is a COALESCE expression index, not a UNIQUE constraint).

use rusqlite::{Connection, OptionalExtension};
use soloist_core::{
    ProjectId, PromptTemplateId, PromptTemplateRepo, PromptTemplateWriteResult, StoreError,
    StoredPromptTemplate,
};

use crate::{sql_err, SqliteStore};

/// The columns every read selects, in order, so [`row_to_template`] decodes one shape.
const TEMPLATE_COLUMNS: &str = "id, project_id, name, description, body, revision";

/// The scope filter, the same expression as the unique index: the global scope (a NULL
/// `project_id`, which `=` can never match) maps to the sentinel 0, which no project rowid
/// ever uses.
const SCOPE_CLAUSE: &str = "COALESCE(project_id, 0) = COALESCE(?1, 0)";

/// The `?1` value [`SCOPE_CLAUSE`] compares against.
fn scope_param(project: Option<ProjectId>) -> Option<i64> {
    project.map(|id| id.get() as i64)
}

impl PromptTemplateRepo for SqliteStore {
    fn write(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<PromptTemplateWriteResult, StoreError> {
        let conn = self.lock();
        // Read the current revision and update-or-insert under one guard, so the guard check
        // and the write cannot interleave with a concurrent writer.
        let current = current_revision(&conn, project, name)?;
        match (current, expected) {
            (Some(revision), Some(expected)) if revision == expected => {
                conn.execute(
                    &format!(
                        "UPDATE prompt_templates SET description = ?3, body = ?4, revision = ?5
                         WHERE {SCOPE_CLAUSE} AND name = ?2"
                    ),
                    (
                        scope_param(project),
                        name,
                        description,
                        body,
                        (revision + 1) as i64,
                    ),
                )
                .map_err(sql_err)?;
                read_one(&conn, project, name)?
                    .map(|stored| PromptTemplateWriteResult::Written(Box::new(stored)))
                    .ok_or_else(|| StoreError::Backend("template vanished after write".into()))
            }
            (None, None) => {
                conn.execute(
                    "INSERT INTO prompt_templates (project_id, name, description, body, revision)
                     VALUES (?1, ?2, ?3, ?4, 1)",
                    (scope_param(project), name, description, body),
                )
                .map_err(sql_err)?;
                read_one(&conn, project, name)?
                    .map(|stored| PromptTemplateWriteResult::Written(Box::new(stored)))
                    .ok_or_else(|| StoreError::Backend("template vanished after create".into()))
            }
            (actual, _) => Ok(PromptTemplateWriteResult::Conflict { actual }),
        }
    }

    fn read(
        &self,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<StoredPromptTemplate>, StoreError> {
        read_one(&self.lock(), project, name)
    }

    fn list(&self, project: Option<ProjectId>) -> Result<Vec<StoredPromptTemplate>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {TEMPLATE_COLUMNS} FROM prompt_templates WHERE {SCOPE_CLAUSE} ORDER BY name"
            ))
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([scope_param(project)], row_to_template)
            .map_err(sql_err)?;
        let mut found = Vec::new();
        for row in rows {
            found.push(row.map_err(sql_err)?);
        }
        Ok(found)
    }

    fn delete(&self, project: Option<ProjectId>, name: &str) -> Result<bool, StoreError> {
        self.lock()
            .execute(
                &format!("DELETE FROM prompt_templates WHERE {SCOPE_CLAUSE} AND name = ?2"),
                (scope_param(project), name),
            )
            .map(|rows| rows > 0)
            .map_err(sql_err)
    }
}

fn read_one(
    conn: &Connection,
    project: Option<ProjectId>,
    name: &str,
) -> Result<Option<StoredPromptTemplate>, StoreError> {
    conn.query_row(
        &format!(
            "SELECT {TEMPLATE_COLUMNS} FROM prompt_templates WHERE {SCOPE_CLAUSE} AND name = ?2"
        ),
        (scope_param(project), name),
        row_to_template,
    )
    .optional()
    .map_err(sql_err)
}

fn current_revision(
    conn: &Connection,
    project: Option<ProjectId>,
    name: &str,
) -> Result<Option<u64>, StoreError> {
    conn.query_row(
        &format!("SELECT revision FROM prompt_templates WHERE {SCOPE_CLAUSE} AND name = ?2"),
        (scope_param(project), name),
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(sql_err)
    .map(|revision| revision.map(|value| value as u64))
}

fn row_to_template(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPromptTemplate> {
    Ok(StoredPromptTemplate {
        id: PromptTemplateId::from_raw(row.get::<_, i64>(0)? as u64),
        project: row
            .get::<_, Option<i64>>(1)?
            .map(|id| ProjectId::from_raw(id as u64)),
        name: row.get(2)?,
        description: row.get(3)?,
        body: row.get(4)?,
        revision: row.get::<_, i64>(5)? as u64,
    })
}

#[cfg(test)]
#[path = "prompt_templates_tests.rs"]
mod tests;
