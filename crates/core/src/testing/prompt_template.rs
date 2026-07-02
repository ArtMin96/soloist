//! In-memory [`PromptTemplateRepo`] fake for headless coordination tests — no real database.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::coordination::{PromptTemplateRepo, PromptTemplateWriteResult, StoredPromptTemplate};
use crate::ids::{ProjectId, PromptTemplateId};
use crate::ports::StoreError;
use crate::sync::lock;

/// An in-memory [`PromptTemplateRepo`] for headless tests. Keyed by `(scope, name)` like the
/// durable table (`None` = the global scope); the whole check-and-write runs under one lock,
/// mirroring the store's atomic revision guard.
#[derive(Default)]
pub struct FakePromptTemplateRepo {
    rows: Mutex<HashMap<(Option<u64>, String), StoredPromptTemplate>>,
    next_id: Mutex<u64>,
}

impl FakePromptTemplateRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PromptTemplateRepo for FakePromptTemplateRepo {
    fn write(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<PromptTemplateWriteResult, StoreError> {
        let mut rows = lock(&self.rows);
        let key = (project.map(ProjectId::get), name.to_owned());
        let current = rows.get(&key);
        match (current, expected) {
            (Some(row), Some(rev)) if row.revision == rev => {
                let updated = StoredPromptTemplate {
                    id: row.id,
                    project,
                    name: name.to_owned(),
                    description: description.map(str::to_owned),
                    body: body.to_owned(),
                    revision: rev + 1,
                };
                rows.insert(key, updated.clone());
                Ok(PromptTemplateWriteResult::Written(Box::new(updated)))
            }
            (None, None) => {
                let mut next_id = lock(&self.next_id);
                *next_id += 1;
                let created = StoredPromptTemplate {
                    id: PromptTemplateId::from_raw(*next_id),
                    project,
                    name: name.to_owned(),
                    description: description.map(str::to_owned),
                    body: body.to_owned(),
                    revision: 1,
                };
                rows.insert(key, created.clone());
                Ok(PromptTemplateWriteResult::Written(Box::new(created)))
            }
            (current, _) => Ok(PromptTemplateWriteResult::Conflict {
                actual: current.map(|row| row.revision),
            }),
        }
    }

    fn read(
        &self,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<StoredPromptTemplate>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(project.map(ProjectId::get), name.to_owned()))
            .cloned())
    }

    fn list(&self, project: Option<ProjectId>) -> Result<Vec<StoredPromptTemplate>, StoreError> {
        let scope = project.map(ProjectId::get);
        let mut rows: Vec<StoredPromptTemplate> = lock(&self.rows)
            .iter()
            .filter(|((row_scope, _), _)| *row_scope == scope)
            .map(|(_, row)| row.clone())
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(rows)
    }

    fn delete(&self, project: Option<ProjectId>, name: &str) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(project.map(ProjectId::get), name.to_owned()))
            .is_some())
    }
}
