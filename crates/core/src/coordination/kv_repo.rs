//! The durable store of the project-scoped key-value map and the port over it (context C6).
//!
//! The kv port is the simplest of the coordination ports: one row per `(project, key)`, no
//! revision guarding, no process ownership. `set` is an upsert (create or replace), `get` returns
//! the current value or `None`, `delete` removes the entry, and `list` returns every pair ordered
//! by key. Values are JSON — callers may store any serializable structure; the port stores and
//! returns the parsed form so the serialization boundary sits in the adapter, not the core.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::ProjectId;
use crate::ports::StoreError;

/// A persisted key-value entry: its key and the stored JSON value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: Value,
}

/// Durable, project-scoped key-value repository. One row per `(project, key)`; the key is the
/// address, and a `set` is an upsert (never partial — the whole value is replaced).
pub trait KvRepo: Send + Sync {
    /// Stores `value` at `key` in `project`, creating or replacing any existing entry.
    fn set(&self, project: ProjectId, key: &str, value: &Value) -> Result<(), StoreError>;

    /// The value at `key` in `project`, or `None` if there is none.
    fn get(&self, project: ProjectId, key: &str) -> Result<Option<Value>, StoreError>;

    /// Removes the entry at `key` in `project`, returning whether one was present.
    fn delete(&self, project: ProjectId, key: &str) -> Result<bool, StoreError>;

    /// Every key-value entry in `project`, ordered by key.
    fn list(&self, project: ProjectId) -> Result<Vec<KvEntry>, StoreError>;
}

/// A [`KvRepo`] that stores nothing — the default until the durable adapter is wired, so the core
/// runs (kv simply never persists) without it. Reads always return empty; writes are silently
/// discarded.
#[derive(Clone, Copy, Default)]
pub struct NoopKvRepo;

impl KvRepo for NoopKvRepo {
    fn set(&self, _project: ProjectId, _key: &str, _value: &Value) -> Result<(), StoreError> {
        Ok(())
    }
    fn get(&self, _project: ProjectId, _key: &str) -> Result<Option<Value>, StoreError> {
        Ok(None)
    }
    fn delete(&self, _project: ProjectId, _key: &str) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn list(&self, _project: ProjectId) -> Result<Vec<KvEntry>, StoreError> {
        Ok(Vec::new())
    }
}
