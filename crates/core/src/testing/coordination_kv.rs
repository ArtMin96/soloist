//! In-memory [`KvRepo`] fake for headless coordination tests — no real database.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::coordination::{KvEntry, KvRepo};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::sync::lock;

/// An in-memory [`KvRepo`] for headless coordination tests. Keyed by `(project_id, key)`,
/// mirrors the upsert/read/delete/list semantics of the durable store without touching SQLite.
#[derive(Default)]
pub struct FakeKvRepo {
    rows: Mutex<HashMap<(u64, String), Value>>,
}

impl FakeKvRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KvRepo for FakeKvRepo {
    fn set(&self, project: ProjectId, key: &str, value: &Value) -> Result<(), StoreError> {
        lock(&self.rows).insert((project.get(), key.to_owned()), value.clone());
        Ok(())
    }

    fn get(&self, project: ProjectId, key: &str) -> Result<Option<Value>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(project.get(), key.to_owned()))
            .cloned())
    }

    fn delete(&self, project: ProjectId, key: &str) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(project.get(), key.to_owned()))
            .is_some())
    }

    fn list(&self, project: ProjectId) -> Result<Vec<KvEntry>, StoreError> {
        let mut entries: Vec<KvEntry> = lock(&self.rows)
            .iter()
            .filter(|((pid, _), _)| *pid == project.get())
            .map(|((_, key), value)| KvEntry {
                key: key.clone(),
                value: value.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(entries)
    }
}
