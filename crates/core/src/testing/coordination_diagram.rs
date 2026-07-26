//! In-memory [`DiagramRepo`] fake for headless coordination tests, mirroring the SQLite store's
//! diagram semantics (atomic revision-guarded write, rename uniqueness, tag read-modify-write) — no
//! real database. Simpler than the scratchpad fake: a diagram has no cross-project transfer and no
//! derived todos, so there is no cascade to model.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::coordination::{DiagramRenameResult, DiagramRepo, DiagramWriteResult, StoredDiagram};
use crate::ids::{DiagramId, ProjectId};
use crate::ports::StoreError;
use crate::sync::lock;

/// An in-memory [`DiagramRepo`] for headless coordination tests. Keyed by `(project, name)` like the
/// durable table, assigns durable ids from a counter, and mirrors the store's atomic revision-guarded
/// write, rename uniqueness, and tag read-modify-write.
#[derive(Default)]
pub struct FakeDiagramRepo {
    rows: Mutex<HashMap<(u64, String), StoredDiagram>>,
    next_id: AtomicU64,
}

impl FakeDiagramRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DiagramRepo for FakeDiagramRepo {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        source: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<DiagramWriteResult, StoreError> {
        let mut rows = lock(&self.rows);
        let slot = (project.get(), name.to_owned());
        match rows.get(&slot) {
            Some(existing) => match expected {
                Some(rev) if rev == existing.revision => {
                    let mut updated = existing.clone();
                    updated.source = source.to_owned();
                    updated.revision = existing.revision + 1;
                    updated.updated_at = now;
                    rows.insert(slot, updated.clone());
                    Ok(DiagramWriteResult::Written(Box::new(updated)))
                }
                _ => Ok(DiagramWriteResult::Conflict {
                    actual: Some(existing.revision),
                }),
            },
            None => match expected {
                None => {
                    let id = DiagramId::from_raw(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
                    let stored = StoredDiagram {
                        id,
                        project,
                        name: name.to_owned(),
                        source: source.to_owned(),
                        tags: Vec::new(),
                        archived: false,
                        revision: 1,
                        updated_at: now,
                    };
                    rows.insert(slot, stored.clone());
                    Ok(DiagramWriteResult::Written(Box::new(stored)))
                }
                Some(_) => Ok(DiagramWriteResult::Conflict { actual: None }),
            },
        }
    }

    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredDiagram>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(project.get(), name.to_owned()))
            .cloned())
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredDiagram>, StoreError> {
        let mut found: Vec<StoredDiagram> = lock(&self.rows)
            .values()
            .filter(|row| row.project == project)
            .cloned()
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(found)
    }

    fn contains(&self, project: ProjectId, id: DiagramId) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .values()
            .any(|row| row.project == project && row.id == id))
    }

    fn rename(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<DiagramRenameResult, StoreError> {
        let mut rows = lock(&self.rows);
        let to_slot = (project.get(), to.to_owned());
        if from != to && rows.contains_key(&to_slot) {
            return Ok(DiagramRenameResult::NameTaken);
        }
        match rows.remove(&(project.get(), from.to_owned())) {
            Some(mut stored) => {
                stored.name = to.to_owned();
                rows.insert(to_slot, stored.clone());
                Ok(DiagramRenameResult::Renamed(Box::new(stored)))
            }
            None => Ok(DiagramRenameResult::NotFound),
        }
    }

    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                for tag in tags {
                    if !stored.tags.contains(tag) {
                        stored.tags.push(tag.clone());
                    }
                }
                stored.tags.sort();
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredDiagram>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                stored.tags.retain(|tag| !tags.contains(tag));
                stored.tags.sort();
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        let distinct: BTreeSet<String> = lock(&self.rows)
            .values()
            .filter(|row| row.project == project)
            .flat_map(|row| row.tags.iter().cloned())
            .collect();
        Ok(distinct.into_iter().collect())
    }

    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredDiagram>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                stored.archived = archived;
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(project.get(), name.to_owned()))
            .is_some())
    }
}
