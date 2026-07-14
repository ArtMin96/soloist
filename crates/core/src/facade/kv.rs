//! Session-scoped kv actions (context C8 → C6): the project-scoped JSON key-value surface a remote
//! caller (MCP today) drives within its effective project.
//!
//! Kv entries are project-scoped durable content with no process ownership, so every method resolves
//! only the session's **effective project** (reusing [`coordination_scope`](Facade::coordination_scope),
//! shared with the other coordination surfaces). Scope is resolved here, in the core, so every remote
//! surface inherits the identical rules.

use serde_json::Value;

use super::coordination::check_payload_size;
use super::Facade;
use crate::coordination::{KvEntry, MAX_KV_VALUE_BYTES};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::SessionId;

impl Facade {
    /// Stores `value` at `key` in the session's effective project, creating or replacing any
    /// existing entry.
    pub fn kv_set(
        &self,
        session: SessionId,
        key: String,
        value: Value,
    ) -> Result<(), CoordinationError> {
        let project = self.coordination_scope(session)?;
        check_payload_size(value.to_string().len(), MAX_KV_VALUE_BYTES, "kv value")?;
        self.kv
            .set(project, &key, &value)
            .map_err(CoordinationError::Store)?;
        self.bus.publish(DomainEvent::KvChanged { project, key });
        Ok(())
    }

    /// The value at `key` in the session's effective project, or `None` if there is none.
    pub fn kv_get(
        &self,
        session: SessionId,
        key: String,
    ) -> Result<Option<Value>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.kv.get(project, &key).map_err(CoordinationError::Store)
    }

    /// Removes the entry at `key` in the session's effective project, returning whether one was
    /// present.
    pub fn kv_delete(&self, session: SessionId, key: String) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let removed = self
            .kv
            .delete(project, &key)
            .map_err(CoordinationError::Store)?;
        if removed {
            self.bus.publish(DomainEvent::KvChanged { project, key });
        }
        Ok(removed)
    }

    /// Every key-value entry in the session's effective project, ordered by key.
    pub fn kv_list(&self, session: SessionId) -> Result<Vec<KvEntry>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.kv.list(project).map_err(CoordinationError::Store)
    }
}

#[cfg(test)]
#[path = "kv_tests.rs"]
mod tests;
