//! In-memory durable-store fakes: a [`FakeTrustRepo`] keyed by `(project, variant)` and a
//! [`FakeProjectRepo`] assigning sequential ids, mirroring the SQLite store's semantics
//! closely enough to exercise the trust gate, config sync, and project-registry logic
//! headless — no real database.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{ProjectRecord, ProjectRepo, StoreError, TrustGrant, TrustRepo};
use crate::sync::lock;

/// An in-memory [`TrustRepo`] keyed by `(project, variant hex)` for command trust and by the
/// project alone for the authorisation to change it, for headless trust and sync tests.
///
/// Provenance is held beside the grant rather than inside it, mirroring the durable store's
/// nullable columns: a variant trusted without provenance reads back as user-authored.
#[derive(Default)]
pub struct FakeTrustRepo {
    trusted: Mutex<HashSet<(u64, String)>>,
    provenance: Mutex<BTreeMap<(u64, String), (ProcessId, String, u64)>>,
    trusted_projects: Mutex<HashSet<u64>>,
}

impl FakeTrustRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// The same repository, with `project` already authorised to be changed — the one line a
    /// test that is not about granting authorisation needs to get past the gate.
    pub fn trusting_project(self, project: ProjectId) -> Self {
        lock(&self.trusted_projects).insert(project.get());
        self
    }
}

impl TrustRepo for FakeTrustRepo {
    fn is_trusted(&self, project: ProjectId, variant: &Hash) -> Result<bool, StoreError> {
        Ok(lock(&self.trusted).contains(&(project.get(), variant.to_hex())))
    }

    fn set_trusted(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        lock(&self.trusted).insert((project.get(), variant.to_hex()));
        Ok(())
    }

    fn set_trusted_with_provenance(
        &self,
        project: ProjectId,
        variant: &Hash,
        requested_by: ProcessId,
        reason: &str,
        granted_at_unix_millis: u64,
    ) -> Result<(), StoreError> {
        lock(&self.trusted).insert((project.get(), variant.to_hex()));
        lock(&self.provenance).insert(
            (project.get(), variant.to_hex()),
            (requested_by, reason.to_owned(), granted_at_unix_millis),
        );
        Ok(())
    }

    fn list_grants(&self, project: ProjectId) -> Result<Vec<TrustGrant>, StoreError> {
        let provenance = lock(&self.provenance);
        Ok(lock(&self.trusted)
            .iter()
            .filter(|(owner, _)| *owner == project.get())
            .map(|key| {
                let recorded = provenance.get(key);
                TrustGrant {
                    variant_hash: key.1.clone(),
                    requested_by: recorded.map(|(process, _, _)| *process),
                    reason: recorded.map(|(_, reason, _)| reason.clone()),
                    granted_at_unix_millis: recorded.map(|(_, _, at)| *at),
                }
            })
            .collect())
    }

    fn revoke(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        lock(&self.trusted).remove(&(project.get(), variant.to_hex()));
        lock(&self.provenance).remove(&(project.get(), variant.to_hex()));
        Ok(())
    }

    fn is_project_trusted(&self, project: ProjectId) -> Result<bool, StoreError> {
        Ok(lock(&self.trusted_projects).contains(&project.get()))
    }

    fn set_project_trusted(&self, project: ProjectId) -> Result<(), StoreError> {
        lock(&self.trusted_projects).insert(project.get());
        Ok(())
    }

    fn revoke_project(&self, project: ProjectId) -> Result<(), StoreError> {
        lock(&self.trusted_projects).remove(&project.get());
        Ok(())
    }
}

struct FakeProjects {
    next_id: u64,
    rows: Vec<ProjectRecord>,
}

/// An in-memory [`ProjectRepo`] assigning sequential ids, for headless registry tests.
/// Mirrors the SQLite store's semantics (canonical-root upsert, cascade-free remove)
/// closely enough to exercise the [`crate::projects::Projects`] logic.
pub struct FakeProjectRepo {
    inner: Mutex<FakeProjects>,
    get_fails: AtomicBool,
}

impl FakeProjectRepo {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FakeProjects {
                next_id: 1,
                rows: Vec::new(),
            }),
            get_fails: AtomicBool::new(false),
        }
    }

    /// Makes [`ProjectRepo::get`] fail with a backend error while `list`/`upsert` keep working,
    /// simulating a transient store fault (a WAL checkpoint or `SQLITE_BUSY`). Lets a test drive
    /// the "scope resolved from memory, name unreadable from the store" path.
    pub fn set_get_failing(&self, failing: bool) {
        self.get_fails.store(failing, Ordering::SeqCst);
    }
}

impl Default for FakeProjectRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectRepo for FakeProjectRepo {
    fn upsert(
        &self,
        root: &Path,
        name: Option<&str>,
        icon: Option<&Path>,
    ) -> Result<ProjectRecord, StoreError> {
        let mut inner = lock(&self.inner);
        if let Some(existing) = inner.rows.iter_mut().find(|r| r.root.as_path() == root) {
            existing.name = name.map(str::to_owned);
            existing.icon = icon.map(Path::to_path_buf);
            return Ok(existing.clone());
        }
        let record = ProjectRecord {
            id: ProjectId::from_raw(inner.next_id),
            root: root.to_path_buf(),
            name: name.map(str::to_owned),
            icon: icon.map(Path::to_path_buf),
        };
        inner.next_id += 1;
        // `rows` is held in display order, as the real store's `position` column keeps it: a
        // newly opened project leads until the user arranges the list otherwise.
        inner.rows.insert(0, record.clone());
        Ok(record)
    }

    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        Ok(lock(&self.inner).rows.clone())
    }

    fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        if self.get_fails.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("simulated store failure".into()));
        }
        Ok(lock(&self.inner).rows.iter().find(|r| r.id == id).cloned())
    }

    fn reorder(&self, order: &[ProjectId]) -> Result<(), StoreError> {
        let mut inner = lock(&self.inner);
        let mut placed: Vec<ProjectRecord> = order
            .iter()
            .filter_map(|id| inner.rows.iter().find(|r| r.id == *id).cloned())
            .collect();
        placed.extend(
            inner
                .rows
                .iter()
                .filter(|r| !order.contains(&r.id))
                .cloned(),
        );
        inner.rows = placed;
        Ok(())
    }

    fn remove(&self, id: ProjectId) -> Result<(), StoreError> {
        lock(&self.inner).rows.retain(|r| r.id != id);
        Ok(())
    }
}
