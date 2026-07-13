//! In-memory durable-store fakes: a [`FakeTrustRepo`] keyed by `(project, variant)` and a
//! [`FakeProjectRepo`] assigning sequential ids, mirroring the SQLite store's semantics
//! closely enough to exercise the trust gate, config sync, and project-registry logic
//! headless — no real database.

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::hash::Hash;
use crate::ids::ProjectId;
use crate::ports::{ProjectRecord, ProjectRepo, StoreError, TrustRepo};
use crate::sync::lock;

/// An in-memory [`TrustRepo`] keyed by `(project, variant hex)`, for headless trust
/// and sync tests.
#[derive(Default)]
pub struct FakeTrustRepo {
    trusted: Mutex<HashSet<(u64, String)>>,
}

impl FakeTrustRepo {
    pub fn new() -> Self {
        Self::default()
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

    fn revoke(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        lock(&self.trusted).remove(&(project.get(), variant.to_hex()));
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
        inner.rows.push(record.clone());
        Ok(record)
    }

    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        Ok(lock(&self.inner).rows.iter().rev().cloned().collect())
    }

    fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        if self.get_fails.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("simulated store failure".into()));
        }
        Ok(lock(&self.inner).rows.iter().find(|r| r.id == id).cloned())
    }

    fn remove(&self, id: ProjectId) -> Result<(), StoreError> {
        lock(&self.inner).rows.retain(|r| r.id != id);
        Ok(())
    }
}
