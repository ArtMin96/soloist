//! In-memory [`SettingsRepo`] fake for headless settings tests — no real database.

use std::sync::Mutex;

use crate::ports::StoreError;
use crate::settings::{Settings, SettingsRepo};
use crate::sync::lock;

/// An in-memory [`SettingsRepo`] for headless tests. Holds the single global record, mirroring the
/// load/save semantics of the durable store without touching SQLite. Starts empty (`load` → `None`)
/// so the aggregate's default-on-absent behaviour is exercised.
#[derive(Default)]
pub struct FakeSettingsRepo {
    record: Mutex<Option<Settings>>,
}

impl FakeSettingsRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SettingsRepo for FakeSettingsRepo {
    fn load(&self) -> Result<Option<Settings>, StoreError> {
        Ok(lock(&self.record).clone())
    }

    fn save(&self, settings: &Settings) -> Result<(), StoreError> {
        *lock(&self.record) = Some(settings.clone());
        Ok(())
    }
}
