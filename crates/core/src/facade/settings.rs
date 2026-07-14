//! Application-settings actions (context C8 → settings): the durable, global preference surface
//! every frontend (the settings UI, and the MCP server reading its tool-group enablement) drives
//! through the one façade.
//!
//! Unlike the coordination surfaces these are **not** project-scoped — settings are global — so the
//! methods take no session and resolve no scope. The first settings are the MCP feature-group
//! toggles (Key-Value defaults off); the policy and persistence live in the [`SettingsStore`]
//! aggregate, so the façade method is a thin pass-through.

use super::Facade;
use crate::ports::StoreError;
use crate::settings::{
    AgentSettings, Appearance, Binding, HotkeyAction, HotkeyBindingView, Integrations,
    McpFeatureGroup, McpToolGroups, Notifications, Sidebar, ToolDefaults,
};

impl Facade {
    /// The Appearance settings — theme + terminal typography. Absent settings read as the
    /// documented defaults.
    pub fn appearance(&self) -> Result<Appearance, StoreError> {
        Ok(self.settings.get(&())?.appearance)
    }

    /// Replaces the Appearance sub-document and persists it, returning the stored value. The whole
    /// tab is saved on any change (auto-save); the write routes through the store's single `update`
    /// primitive, so the frontend, CLI, and any other front drive the same record.
    pub fn set_appearance(&self, appearance: Appearance) -> Result<Appearance, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.appearance = appearance)?
            .appearance)
    }

    /// The Sidebar settings — what the process-tree sidebar shows.
    pub fn sidebar_settings(&self) -> Result<Sidebar, StoreError> {
        Ok(self.settings.get(&())?.sidebar)
    }

    /// Replaces the Sidebar sub-document and persists it (auto-save), returning the stored value.
    pub fn set_sidebar_settings(&self, sidebar: Sidebar) -> Result<Sidebar, StoreError> {
        Ok(self.settings.update(&(), |s| s.sidebar = sidebar)?.sidebar)
    }

    /// The hotkey keymap read model — every action with its scope, effective binding, and whether
    /// it is still the code default. The defaults are code-defined; only overrides persist.
    pub fn hotkeys(&self) -> Result<Vec<HotkeyBindingView>, StoreError> {
        Ok(self.settings.get(&())?.hotkeys.view())
    }

    /// Remaps one action to a new chord and persists it, returning the updated keymap.
    pub fn remap_hotkey(
        &self,
        action: HotkeyAction,
        binding: Binding,
    ) -> Result<Vec<HotkeyBindingView>, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.hotkeys.remap(action, binding))?
            .hotkeys
            .view())
    }

    /// Disables one action (it keeps no binding until reset) and persists it.
    pub fn disable_hotkey(
        &self,
        action: HotkeyAction,
    ) -> Result<Vec<HotkeyBindingView>, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.hotkeys.disable(action))?
            .hotkeys
            .view())
    }

    /// Resets one action to its code default (drops its override) and persists it.
    pub fn reset_hotkey(&self, action: HotkeyAction) -> Result<Vec<HotkeyBindingView>, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.hotkeys.reset(action))?
            .hotkeys
            .view())
    }

    /// Resets every action to its code default ("Reset all to defaults") and persists it.
    pub fn reset_all_hotkeys(&self) -> Result<Vec<HotkeyBindingView>, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.hotkeys.reset_all())?
            .hotkeys
            .view())
    }

    /// The Agents settings — the auto-summarization opt-in (the tool registry itself is C4).
    pub fn agent_settings(&self) -> Result<AgentSettings, StoreError> {
        Ok(self.settings.get(&())?.agents)
    }

    /// Replaces the Agents sub-document and persists it (auto-save), returning the stored value.
    pub fn set_agent_settings(&self, agents: AgentSettings) -> Result<AgentSettings, StoreError> {
        Ok(self.settings.update(&(), |s| s.agents = agents)?.agents)
    }

    /// The Tools settings — the default editor and terminal.
    pub fn tool_defaults(&self) -> Result<ToolDefaults, StoreError> {
        Ok(self.settings.get(&())?.tools)
    }

    /// Replaces the Tools sub-document and persists it (auto-save), returning the stored value.
    pub fn set_tool_defaults(&self, tools: ToolDefaults) -> Result<ToolDefaults, StoreError> {
        Ok(self.settings.update(&(), |s| s.tools = tools)?.tools)
    }

    /// The Integrations settings — the MCP and HTTP-API master toggles. The per-group MCP enablement
    /// is [`Self::mcp_tool_groups`].
    pub fn integration_settings(&self) -> Result<Integrations, StoreError> {
        Ok(self.settings.get(&())?.integrations)
    }

    /// Replaces the Integrations sub-document and persists it (auto-save), returning the stored value.
    pub fn set_integration_settings(
        &self,
        integrations: Integrations,
    ) -> Result<Integrations, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.integrations = integrations)?
            .integrations)
    }

    /// The Notifications settings — the master on/off the notification reactor consults before any
    /// toast. Off silences notifications everywhere; the per-project crash/exit and terminal-alert
    /// switches ([`Self::project_settings`]) refine what an enabled reactor shows. Absent settings
    /// read as the documented default (on).
    pub fn notification_settings(&self) -> Result<Notifications, StoreError> {
        Ok(self.settings.get(&())?.notifications)
    }

    /// Replaces the Notifications sub-document and persists it (auto-save), returning the stored
    /// value. The reactor reads the same durable record, so the master switch takes effect on the
    /// next event without a restart.
    pub fn set_notification_settings(
        &self,
        notifications: Notifications,
    ) -> Result<Notifications, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.notifications = notifications)?
            .notifications)
    }

    /// The MCP feature-group enablement — the read the MCP server consults to decide which
    /// feature-tool groups to serve (core groups are always served). Absent settings read as the
    /// documented defaults.
    pub fn mcp_tool_groups(&self) -> Result<McpToolGroups, StoreError> {
        Ok(self.settings.get(&())?.mcp_tool_groups)
    }

    /// Enables or disables one MCP feature group and persists it, returning the updated enablement.
    /// One method behind the façade, so a settings UI, the CLI, or an MCP tool all toggle the same
    /// durable record. Routes through the generic store's single `update` write primitive.
    pub fn set_mcp_tool_group(
        &self,
        group: McpFeatureGroup,
        enabled: bool,
    ) -> Result<McpToolGroups, StoreError> {
        Ok(self
            .settings
            .update(&(), |s| s.mcp_tool_groups.set(group, enabled))?
            .mcp_tool_groups)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
