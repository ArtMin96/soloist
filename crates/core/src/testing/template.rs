//! In-memory [`TemplateRepo`] fake for headless coordination tests — no real database.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::coordination::{StoredTemplate, TemplateRepo, TemplateWriteResult};
use crate::ids::{ProjectId, TemplateId};
use crate::ports::StoreError;
use crate::sync::lock;
use crate::template::TemplateKind;

/// The fake's row map key: `(kind, scope, name)`, mirroring the durable table's addressing
/// (`None` scope = the global scope).
type RowKey = (TemplateKind, Option<u64>, String);

/// An in-memory [`TemplateRepo`] for headless tests. Keyed by `(kind, scope, name)` like the
/// durable table; the whole check-and-write runs under one lock, mirroring the store's atomic
/// revision guard. `list_calls` counts the reads that reach the store, so a test can prove the
/// aggregate's cache absorbs later reads.
#[derive(Default)]
pub struct FakeTemplateRepo {
    rows: Mutex<HashMap<RowKey, StoredTemplate>>,
    next_id: Mutex<u64>,
    list_calls: AtomicUsize,
}

impl FakeTemplateRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many times `list` reached the store — every cache miss, none of the hits.
    pub fn list_calls(&self) -> usize {
        self.list_calls.load(Ordering::Relaxed)
    }
}

impl TemplateRepo for FakeTemplateRepo {
    fn write(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<TemplateWriteResult, StoreError> {
        let mut rows = lock(&self.rows);
        let key = (kind, project.map(ProjectId::get), name.to_owned());
        let current = rows.get(&key);
        match (current, expected) {
            (Some(row), Some(rev)) if row.revision == rev => {
                let updated = StoredTemplate {
                    id: row.id,
                    kind,
                    project,
                    name: name.to_owned(),
                    description: description.map(str::to_owned),
                    body: body.to_owned(),
                    revision: rev + 1,
                };
                rows.insert(key, updated.clone());
                Ok(TemplateWriteResult::Written(Box::new(updated)))
            }
            (None, None) => {
                let mut next_id = lock(&self.next_id);
                *next_id += 1;
                let created = StoredTemplate {
                    id: TemplateId::from_raw(*next_id),
                    kind,
                    project,
                    name: name.to_owned(),
                    description: description.map(str::to_owned),
                    body: body.to_owned(),
                    revision: 1,
                };
                rows.insert(key, created.clone());
                Ok(TemplateWriteResult::Written(Box::new(created)))
            }
            (current, _) => Ok(TemplateWriteResult::Conflict {
                actual: current.map(|row| row.revision),
            }),
        }
    }

    fn read(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<StoredTemplate>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(kind, project.map(ProjectId::get), name.to_owned()))
            .cloned())
    }

    fn list(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
    ) -> Result<Vec<StoredTemplate>, StoreError> {
        self.list_calls.fetch_add(1, Ordering::Relaxed);
        let scope = project.map(ProjectId::get);
        let mut rows: Vec<StoredTemplate> = lock(&self.rows)
            .iter()
            .filter(|((row_kind, row_scope, _), _)| *row_kind == kind && *row_scope == scope)
            .map(|(_, row)| row.clone())
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(rows)
    }

    fn delete(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(kind, project.map(ProjectId::get), name.to_owned()))
            .is_some())
    }
}
