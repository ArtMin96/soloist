//! The key-value aggregate (context C6): durable, project-scoped JSON state agents store and share.
//!
//! Unlike leases and timers, kv entries are not process-owned — they survive an app restart and are
//! never cleared by launch reconciliation. Unlike scratchpads and todos, they have no revision
//! guarding and no typed document structure: a value is any JSON, small structured state that does
//! not warrant a richer abstraction. The aggregate is intentionally thin — the domain rule is just
//! project scope — and delegates all persistence to the [`KvRepo`](super::KvRepo) port.

use std::sync::Arc;

use serde_json::Value;

use super::kv_repo::{KvEntry, KvRepo};
use crate::ids::ProjectId;
use crate::ports::StoreError;

/// The key-value aggregate. Wraps the port and provides the project-scoped CRUD surface; the
/// `Facade` owns one instance and resolves session scope before calling here.
pub struct Kv {
    repo: Arc<dyn KvRepo>,
}

impl Kv {
    pub fn new(repo: Arc<dyn KvRepo>) -> Self {
        Self { repo }
    }

    /// Stores `value` at `key` in `project`, creating or replacing any existing entry.
    pub fn set(&self, project: ProjectId, key: &str, value: &Value) -> Result<(), StoreError> {
        self.repo.set(project, key, value)
    }

    /// The value at `key` in `project`, or `None` if there is none.
    pub fn get(&self, project: ProjectId, key: &str) -> Result<Option<Value>, StoreError> {
        self.repo.get(project, key)
    }

    /// Removes the entry at `key` in `project`, returning whether one was present.
    pub fn delete(&self, project: ProjectId, key: &str) -> Result<bool, StoreError> {
        self.repo.delete(project, key)
    }

    /// Every key-value entry in `project`, ordered by key.
    pub fn list(&self, project: ProjectId) -> Result<Vec<KvEntry>, StoreError> {
        self.repo.list(project)
    }
}

#[cfg(test)]
#[path = "kv_tests.rs"]
mod tests;
