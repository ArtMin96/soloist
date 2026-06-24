//! Durable application settings (a focused context): user preferences that persist across runs,
//! distinct from `solo.yml` project config (C1) and from ephemeral runtime state.
//!
//! The first setting is the per-group MCP tool enablement. The MCP server's core tool groups are
//! always served; the feature groups can be toggled — Scratchpads, Todos and Timers default on,
//! Key-Value defaults off. The settings document is a single global record; the [`SettingsRepo`]
//! port persists it (SQLite in the app, nothing under the `Noop` default), and the [`SettingsStore`]
//! aggregate applies the defaults so an absent record reads as the documented defaults. The
//! document carries serde defaults, so a build that adds a setting still reads a record an older
//! build wrote.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ports::StoreError;

/// A toggleable MCP feature-tool group. The core groups (Project, Process, Output, Bulk,
/// Services, Agent/Terminal, Coordination leases, Setup) are always served and are not
/// represented here; only the feature groups can be turned off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpFeatureGroup {
    Scratchpads,
    Todos,
    Timers,
    KeyValue,
}

impl McpFeatureGroup {
    /// Every toggleable feature group, in display order — the single source the settings document
    /// and the MCP server iterate, so adding a group is one edit here plus the exhaustive matches
    /// in [`McpToolGroups`].
    pub const ALL: [McpFeatureGroup; 4] = [
        McpFeatureGroup::Scratchpads,
        McpFeatureGroup::Todos,
        McpFeatureGroup::Timers,
        McpFeatureGroup::KeyValue,
    ];
}

/// Which MCP feature-tool groups the server exposes. Scratchpads, Todos and Timers default on;
/// Key-Value defaults off. `#[serde(default)]` fills any field an older record omits from
/// [`Default`], so adding a group stays backward-compatible with stored records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpToolGroups {
    pub scratchpads: bool,
    pub todos: bool,
    pub timers: bool,
    pub key_value: bool,
}

impl Default for McpToolGroups {
    fn default() -> Self {
        Self {
            scratchpads: true,
            todos: true,
            timers: true,
            key_value: false,
        }
    }
}

impl McpToolGroups {
    /// Whether `group` is currently enabled.
    pub fn enabled(&self, group: McpFeatureGroup) -> bool {
        match group {
            McpFeatureGroup::Scratchpads => self.scratchpads,
            McpFeatureGroup::Todos => self.todos,
            McpFeatureGroup::Timers => self.timers,
            McpFeatureGroup::KeyValue => self.key_value,
        }
    }

    /// Sets `group`'s enablement in place.
    pub fn set(&mut self, group: McpFeatureGroup, enabled: bool) {
        match group {
            McpFeatureGroup::Scratchpads => self.scratchpads = enabled,
            McpFeatureGroup::Todos => self.todos = enabled,
            McpFeatureGroup::Timers => self.timers = enabled,
            McpFeatureGroup::KeyValue => self.key_value = enabled,
        }
    }
}

/// The durable settings document: one global record (not project-scoped). Every field carries a
/// serde default so a record an older build wrote still deserializes after a field is added.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mcp_tool_groups: McpToolGroups,
}

/// Durable settings repository: loads and saves the single global [`Settings`] record. `load`
/// returns `None` when nothing has been stored yet (a fresh install); the aggregate maps that to
/// the defaults. `save` replaces the whole record.
pub trait SettingsRepo: Send + Sync {
    /// The stored settings record, or `None` when none has been written yet.
    fn load(&self) -> Result<Option<Settings>, StoreError>;

    /// Stores `settings`, replacing any existing record.
    fn save(&self, settings: &Settings) -> Result<(), StoreError>;
}

/// A [`SettingsRepo`] that stores nothing — the default until the durable adapter is wired, so the
/// core runs (settings stay at their defaults) without it. `load` always returns `None`; `save` is
/// discarded.
#[derive(Clone, Copy, Default)]
pub struct NoopSettingsRepo;

impl SettingsRepo for NoopSettingsRepo {
    fn load(&self) -> Result<Option<Settings>, StoreError> {
        Ok(None)
    }
    fn save(&self, _settings: &Settings) -> Result<(), StoreError> {
        Ok(())
    }
}

/// The settings aggregate: reads and updates the durable [`Settings`] record through the port,
/// applying the documented defaults so an absent record reads as the defaults. The `Facade` owns
/// one instance (mirrors [`TrustStore`](crate::trust::TrustStore) over its repo).
pub struct SettingsStore {
    repo: Arc<dyn SettingsRepo>,
}

impl SettingsStore {
    pub fn new(repo: Arc<dyn SettingsRepo>) -> Self {
        Self { repo }
    }

    /// The current settings — the stored record, or the defaults if none has been stored.
    pub fn get(&self) -> Result<Settings, StoreError> {
        Ok(self.repo.load()?.unwrap_or_default())
    }

    /// The current MCP feature-group enablement — the read the MCP server consults to decide which
    /// feature-tool groups to serve.
    pub fn mcp_tool_groups(&self) -> Result<McpToolGroups, StoreError> {
        Ok(self.get()?.mcp_tool_groups)
    }

    /// Sets one MCP feature group's enablement and persists the whole record. Returns the updated
    /// group enablement.
    pub fn set_mcp_tool_group(
        &self,
        group: McpFeatureGroup,
        enabled: bool,
    ) -> Result<McpToolGroups, StoreError> {
        let mut settings = self.get()?;
        settings.mcp_tool_groups.set(group, enabled);
        self.repo.save(&settings)?;
        Ok(settings.mcp_tool_groups)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
