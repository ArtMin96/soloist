//! Durable application settings (a focused context): user preferences that persist across runs,
//! distinct from `solo.yml` project config (C1, [`Visibility::Shared`](crate::config)) and from
//! ephemeral runtime state.
//!
//! One generic base serves every settings surface. A [`SettingsStore<K, D>`] reads and writes a
//! serde-default document `D` keyed by `K` through a [`SettingsRepo<K, D>`] port, applying the
//! document defaults so an absent record reads as the defaults. The key selects the surface: the
//! global preferences are [`Settings`] keyed by `()` (one singleton record); a per-project local
//! document is keyed by [`ProjectId`](crate::ids::ProjectId) over the same base. Adding a setting
//! is one `#[serde(default)]` field plus one façade getter/setter — never a new store. Because the
//! document carries serde defaults, a build that adds a field still reads a record an older build
//! wrote.
//!
//! The first global setting is the per-group MCP tool enablement: the MCP server's core tool groups
//! are always served, while the feature groups can be toggled — Scratchpads, Todos and Timers
//! default on, Key-Value defaults off.

use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::ports::StoreError;

mod agents;
mod appearance;
mod hotkeys;
mod integrations;
mod notifications;
mod project;
mod sidebar;
mod tools;

pub use agents::AgentSettings;
pub use appearance::{
    Appearance, FontScale, FontWeight, LetterSpacing, LineHeight, TerminalAppearance, Theme,
};
pub use hotkeys::{Binding, HotkeyAction, HotkeyBindingView, HotkeyScope, Hotkeys};
pub use integrations::Integrations;
pub use notifications::Notifications;
pub use project::ProjectSettings;
pub use sidebar::{
    ProcessCpuThreshold, ProcessMemThreshold, ProjectCpuThreshold, ProjectMemThreshold, Sidebar,
};
pub use tools::ToolDefaults;

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
    PromptTemplates,
}

impl McpFeatureGroup {
    /// Every toggleable feature group, in display order — the single source the settings document
    /// and the MCP server iterate, so adding a group is one edit here plus the exhaustive matches
    /// in [`McpToolGroups`].
    pub const ALL: [McpFeatureGroup; 5] = [
        McpFeatureGroup::Scratchpads,
        McpFeatureGroup::Todos,
        McpFeatureGroup::Timers,
        McpFeatureGroup::KeyValue,
        McpFeatureGroup::PromptTemplates,
    ];

    /// The group's human-readable name — what the settings UI shows and the agent guide
    /// renders, so the toggleable-group list can never drift from the enum.
    pub fn label(self) -> &'static str {
        match self {
            McpFeatureGroup::Scratchpads => "Scratchpads",
            McpFeatureGroup::Todos => "Todos",
            McpFeatureGroup::Timers => "Timers",
            McpFeatureGroup::KeyValue => "Key-Value",
            McpFeatureGroup::PromptTemplates => "Prompt Templates",
        }
    }
}

/// Which MCP feature-tool groups the server exposes. Scratchpads, Todos and Timers default on;
/// Key-Value and Prompt Templates default off. `#[serde(default)]` fills any field an older
/// record omits from [`Default`], so adding a group stays backward-compatible with stored records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpToolGroups {
    pub scratchpads: bool,
    pub todos: bool,
    pub timers: bool,
    pub key_value: bool,
    pub prompt_templates: bool,
}

impl Default for McpToolGroups {
    fn default() -> Self {
        Self {
            scratchpads: true,
            todos: true,
            timers: true,
            key_value: false,
            prompt_templates: false,
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
            McpFeatureGroup::PromptTemplates => self.prompt_templates,
        }
    }

    /// Sets `group`'s enablement in place.
    pub fn set(&mut self, group: McpFeatureGroup, enabled: bool) {
        match group {
            McpFeatureGroup::Scratchpads => self.scratchpads = enabled,
            McpFeatureGroup::Todos => self.todos = enabled,
            McpFeatureGroup::Timers => self.timers = enabled,
            McpFeatureGroup::KeyValue => self.key_value = enabled,
            McpFeatureGroup::PromptTemplates => self.prompt_templates = enabled,
        }
    }
}

/// The durable global settings document: one record (not project-scoped) holding one sub-document
/// per Settings tab. Every field carries a serde default so a record an older build wrote still
/// deserializes after a tab or field is added — the heart of the "add a field, not a store" recipe.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: Appearance,
    pub sidebar: Sidebar,
    pub hotkeys: Hotkeys,
    pub agents: AgentSettings,
    pub tools: ToolDefaults,
    pub integrations: Integrations,
    pub notifications: Notifications,
    pub mcp_tool_groups: McpToolGroups,
}

/// Durable settings repository: loads and saves a document `D` keyed by `K`. `load` returns `None`
/// when nothing has been stored for that key yet (a fresh install); the aggregate maps that to the
/// defaults. `save` replaces the whole record. `K = ()` selects the global singleton record;
/// `K = ProjectId` selects one project's local record. The generic parameters keep the trait
/// object-safe, so an adapter is held as `Arc<dyn SettingsRepo<K, D>>`.
pub trait SettingsRepo<K, D>: Send + Sync {
    /// The stored record for `key`, or `None` when none has been written yet.
    fn load(&self, key: &K) -> Result<Option<D>, StoreError>;

    /// Stores `value` under `key`, replacing any existing record.
    fn save(&self, key: &K, value: &D) -> Result<(), StoreError>;
}

/// A [`SettingsRepo`] that stores nothing — the default until the durable adapter is wired, so the
/// core runs (settings stay at their defaults) without it. `load` always returns `None`; `save` is
/// discarded. One value implements the port for every surface (`K`/`D` pair).
#[derive(Clone, Copy, Default)]
pub struct NoopSettingsRepo;

impl<K, D> SettingsRepo<K, D> for NoopSettingsRepo {
    fn load(&self, _key: &K) -> Result<Option<D>, StoreError> {
        Ok(None)
    }
    fn save(&self, _key: &K, _value: &D) -> Result<(), StoreError> {
        Ok(())
    }
}

/// The settings aggregate: reads and updates a durable document `D` keyed by `K` through the port,
/// applying the document defaults so an absent record reads as the defaults. The same base serves
/// every surface — the `Facade` owns a `SettingsStore<(), Settings>` for global preferences, and a
/// `SettingsStore<ProjectId, ProjectSettings>` for per-project local ones — so neither re-rolls
/// persistence (mirrors [`TrustStore`](crate::trust::TrustStore) over its repo).
pub struct SettingsStore<K, D> {
    repo: Arc<dyn SettingsRepo<K, D>>,
    /// Serializes the read-modify-write so two concurrent [`update`](Self::update)s on this store
    /// can't lose one another's change. The record is a single row holding one document; a setter
    /// replaces a whole sub-document, so an interleaved read-then-write would clobber a change made
    /// between the read and the save. Holding this across the whole cycle makes the write atomic
    /// even when several fronts (the settings UI, the CLI, an MCP tool) drive the same record.
    write_lock: Mutex<()>,
}

impl<K, D: Default> SettingsStore<K, D> {
    pub fn new(repo: Arc<dyn SettingsRepo<K, D>>) -> Self {
        Self {
            repo,
            write_lock: Mutex::new(()),
        }
    }

    /// The current document for `key` — the stored record, or the defaults if none has been stored.
    pub fn get(&self, key: &K) -> Result<D, StoreError> {
        Ok(self.repo.load(key)?.unwrap_or_default())
    }

    /// The single write primitive: read the current document, apply one `mutator`, persist the whole
    /// record, and return the updated document. Every façade setter routes through this, so there is
    /// one place a settings write happens. The read-modify-write is serialized by `write_lock`, so
    /// concurrent setters apply in turn rather than clobbering each other.
    pub fn update(&self, key: &K, mutator: impl FnOnce(&mut D)) -> Result<D, StoreError> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut value = self.get(key)?;
        mutator(&mut value);
        self.repo.save(key, &value)?;
        Ok(value)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
