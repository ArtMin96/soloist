//! In-memory generic [`SettingsRepo`] fake for headless settings tests — no real database.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;

use crate::ports::StoreError;
use crate::settings::SettingsRepo;
use crate::sync::lock;

/// An in-memory [`SettingsRepo<K, D>`] for headless tests. Holds one record per key in a map,
/// mirroring the load/save semantics of the durable store without touching SQLite. Starts empty
/// (`load` → `None`) so the aggregate's default-on-absent behaviour is exercised. The same fake
/// serves the global (`K = ()`) and per-project (`K = ProjectId`) surfaces, so a test never
/// re-rolls a settings fake.
pub struct FakeSettingsRepo<K, D> {
    records: Mutex<HashMap<K, D>>,
}

impl<K, D> Default for FakeSettingsRepo<K, D> {
    fn default() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, D> FakeSettingsRepo<K, D> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K, D> SettingsRepo<K, D> for FakeSettingsRepo<K, D>
where
    K: Eq + Hash + Clone + Send + Sync,
    D: Clone + Send + Sync,
{
    fn load(&self, key: &K) -> Result<Option<D>, StoreError> {
        Ok(lock(&self.records).get(key).cloned())
    }

    fn save(&self, key: &K, value: &D) -> Result<(), StoreError> {
        lock(&self.records).insert(key.clone(), value.clone());
        Ok(())
    }
}
