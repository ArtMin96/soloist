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
use crate::settings::{McpFeatureGroup, McpToolGroups};

impl Facade {
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
