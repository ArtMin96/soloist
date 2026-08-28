//! The coordination diagram repository — the core [`DiagramRepo`] port.
//!
//! One row per `(project_id, name)` in the `diagrams` table, identified durably by the
//! `AUTOINCREMENT` id (never reused, so a renamed diagram keeps its identity). The free-form
//! Mermaid body is stored as-is in the `source` column and the tag set as a JSON array in `tags`.
//! Like a scratchpad a diagram is durable and **not** process-owned: it survives an app restart,
//! so there is no launch-reconcile clear. The `project_id` foreign key cascades, so removing a
//! project drops its diagrams. Unlike a scratchpad, a diagram has no cross-project transfer.
//!
//! Every operation's guard, conflict, decode, and tag read-modify-write mechanics are shared with
//! [`scratchpads`](crate::scratchpads) over [`doc_table`](crate::doc_table); this module keeps
//! only this table's statements and its row shape.

use rusqlite::Row;
use soloist_core::{
    DiagramId, DiagramRenameResult, DiagramRepo, DiagramWriteResult, ProjectId, StoreError,
    StoredDiagram,
};

use crate::doc_table::{self, DocRename, DocStatements, DocTable, DocWrite, TaggedRow};
use crate::SqliteStore;

/// The noun a "row vanished" error and a tag (de)serialize error name for this table.
const LABEL: &str = "diagram";

static STATEMENTS: DocStatements = DocStatements {
    label: LABEL,
    select_revision: "SELECT revision FROM diagrams WHERE project_id = ?1 AND name = ?2",
    select_one: "SELECT id, project_id, name, source, tags, archived, revision, updated_at \
                 FROM diagrams WHERE project_id = ?1 AND name = ?2",
    select_list: "SELECT id, project_id, name, source, tags, archived, revision, updated_at \
                  FROM diagrams WHERE project_id = ?1 ORDER BY name",
    select_tags: "SELECT tags FROM diagrams WHERE project_id = ?1",
    select_contains: "SELECT 1 FROM diagrams WHERE project_id = ?1 AND id = ?2",
    insert: "INSERT INTO diagrams (project_id, name, source, tags, archived, revision, \
             updated_at) VALUES (?1, ?2, ?3, '[]', 0, 1, ?4)",
    update_body: "UPDATE diagrams SET source = ?3, revision = ?4, updated_at = ?5 \
                  WHERE project_id = ?1 AND name = ?2",
    update_name: "UPDATE diagrams SET name = ?3 WHERE project_id = ?1 AND name = ?2",
    update_tags: "UPDATE diagrams SET tags = ?3 WHERE project_id = ?1 AND name = ?2",
    update_archived: "UPDATE diagrams SET archived = ?3 WHERE project_id = ?1 AND name = ?2",
    delete: "DELETE FROM diagrams WHERE project_id = ?1 AND name = ?2",
};

fn table() -> DocTable<'static, StoredDiagram> {
    DocTable::new(&STATEMENTS, row_to_diagram)
}

impl TaggedRow for StoredDiagram {
    fn tags_mut(&mut self) -> &mut Vec<String> {
        &mut self.tags
    }
}

impl DiagramRepo for SqliteStore {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        source: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<DiagramWriteResult, StoreError> {
        let conn = self.lock();
        match table().write(&conn, project, name, source, expected, now)? {
            DocWrite::Written(stored) => Ok(DiagramWriteResult::Written(stored)),
            DocWrite::Conflict { actual } => Ok(DiagramWriteResult::Conflict { actual }),
        }
    }

    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredDiagram>, StoreError> {
        table().read_one(&self.lock(), project, name)
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredDiagram>, StoreError> {
        table().list(&self.lock(), project)
    }

    fn contains(&self, project: ProjectId, id: DiagramId) -> Result<bool, StoreError> {
        table().contains(&self.lock(), project, id.get())
    }

    fn rename(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<DiagramRenameResult, StoreError> {
        let conn = self.lock();
        match table().rename(&conn, project, from, to)? {
            DocRename::Renamed(stored) => Ok(DiagramRenameResult::Renamed(stored)),
            DocRename::NotFound => Ok(DiagramRenameResult::NotFound),
            DocRename::NameTaken => Ok(DiagramRenameResult::NameTaken),
        }
    }

    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError> {
        table().update_tags(&self.lock(), project, name, |current| {
            for tag in tags {
                if !current.contains(tag) {
                    current.push(tag.clone());
                }
            }
        })
    }

    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError> {
        table().update_tags(&self.lock(), project, name, |current| {
            current.retain(|tag| !tags.contains(tag))
        })
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        table().tags(&self.lock(), project)
    }

    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredDiagram>, StoreError> {
        table().set_archived(&self.lock(), project, name, archived)
    }

    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        table().delete(&self.lock(), project, name)
    }
}

/// Decodes one row into a [`StoredDiagram`]. The `source` column is the raw Mermaid body, read
/// as-is.
fn row_to_diagram(row: &Row<'_>) -> rusqlite::Result<Result<StoredDiagram, StoreError>> {
    let id: i64 = row.get(0)?;
    let project: i64 = row.get(1)?;
    let name: String = row.get(2)?;
    let source: String = row.get(3)?;
    let tags_json: String = row.get(4)?;
    let archived: i64 = row.get(5)?;
    let revision: i64 = row.get(6)?;
    let updated_at: i64 = row.get(7)?;
    Ok(
        doc_table::decode_tags(&tags_json, LABEL).map(|tags| StoredDiagram {
            id: DiagramId::from_raw(id as u64),
            project: ProjectId::from_raw(project as u64),
            name,
            source,
            tags,
            archived: archived != 0,
            revision: revision as u64,
            updated_at: updated_at as u64,
        }),
    )
}

#[cfg(test)]
#[path = "diagrams_tests.rs"]
mod tests;
