//! Integrations settings (global Integrations tab): the master on/off for the two local integration
//! surfaces — the MCP server (stdio, D4) and the loopback HTTP API (`127.0.0.1:24678`, H1).
//!
//! The per-group MCP tool enablement is a separate field on the settings document
//! ([`McpToolGroups`](super::McpToolGroups), the G10 work already landed); this document holds only
//! the two master toggles the tab shows. These are persisted preferences; the surface each gates
//! (the MCP server's served tools, the HTTP server) is wired in the respective adapter.

use serde::{Deserialize, Serialize};

/// The Integrations tab document — the two master integration toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Integrations {
    /// Allow AI assistants to control processes over the MCP server (stdio). Off hides the whole MCP
    /// surface; the per-group toggles ([`McpToolGroups`](super::McpToolGroups)) refine what an
    /// enabled server exposes.
    pub mcp_enabled: bool,
    /// Expose the loopback REST API on `127.0.0.1:24678` for local tools.
    pub http_api_enabled: bool,
}

impl Default for Integrations {
    fn default() -> Self {
        Self {
            mcp_enabled: true,
            http_api_enabled: true,
        }
    }
}
