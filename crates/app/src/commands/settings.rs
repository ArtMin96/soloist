//! The durable-settings command surface: one thin wrapper per [`Facade`] settings method.
//!
//! Each command marshals webview arguments, calls the one core method, and maps the typed
//! [`StoreError`](soloist_core::StoreError) to a string the UI renders. The whole-tab setters
//! auto-save (the core persists the document on every change) and return the stored value, so
//! the frontend reflects exactly what was written. No policy lives here — the settings store
//! is the single source, driven identically by every front.

use std::sync::Arc;

use soloist_core::{
    AgentSettings, Appearance, Binding, Facade, HotkeyAction, HotkeyBindingView, Integrations,
    McpFeatureGroup, McpToolGroups, Sidebar, ToolDefaults,
};
use tauri::State;

/// The Appearance settings — theme and terminal typography.
#[tauri::command]
pub async fn appearance(facade: State<'_, Arc<Facade>>) -> Result<Appearance, String> {
    facade.appearance().map_err(|err| err.to_string())
}

/// Replaces the Appearance sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_appearance(
    appearance: Appearance,
    facade: State<'_, Arc<Facade>>,
) -> Result<Appearance, String> {
    facade
        .set_appearance(appearance)
        .map_err(|err| err.to_string())
}

/// The Sidebar settings — what the process-tree sidebar shows.
#[tauri::command]
pub async fn sidebar_settings(facade: State<'_, Arc<Facade>>) -> Result<Sidebar, String> {
    facade.sidebar_settings().map_err(|err| err.to_string())
}

/// Replaces the Sidebar sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_sidebar_settings(
    sidebar: Sidebar,
    facade: State<'_, Arc<Facade>>,
) -> Result<Sidebar, String> {
    facade
        .set_sidebar_settings(sidebar)
        .map_err(|err| err.to_string())
}

/// The hotkey keymap read model — every action with its scope, effective binding, and whether
/// it is still the code default.
#[tauri::command]
pub async fn hotkeys(facade: State<'_, Arc<Facade>>) -> Result<Vec<HotkeyBindingView>, String> {
    facade.hotkeys().map_err(|err| err.to_string())
}

/// Remaps one action to a new chord (auto-save), returning the updated keymap.
#[tauri::command]
pub async fn remap_hotkey(
    action: HotkeyAction,
    binding: Binding,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade
        .remap_hotkey(action, binding)
        .map_err(|err| err.to_string())
}

/// Disables one action — hover-and-press-x — until it is reset (auto-save).
#[tauri::command]
pub async fn disable_hotkey(
    action: HotkeyAction,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade.disable_hotkey(action).map_err(|err| err.to_string())
}

/// Resets one action to its code default by dropping its override (auto-save).
#[tauri::command]
pub async fn reset_hotkey(
    action: HotkeyAction,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade.reset_hotkey(action).map_err(|err| err.to_string())
}

/// Resets every action to its code default — "Reset all to defaults" (auto-save).
#[tauri::command]
pub async fn reset_all_hotkeys(
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade.reset_all_hotkeys().map_err(|err| err.to_string())
}

/// The Agents settings — the auto-summarization opt-in.
#[tauri::command]
pub async fn agent_settings(facade: State<'_, Arc<Facade>>) -> Result<AgentSettings, String> {
    facade.agent_settings().map_err(|err| err.to_string())
}

/// Replaces the Agents sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_agent_settings(
    agents: AgentSettings,
    facade: State<'_, Arc<Facade>>,
) -> Result<AgentSettings, String> {
    facade
        .set_agent_settings(agents)
        .map_err(|err| err.to_string())
}

/// The Tools settings — the default editor and terminal.
#[tauri::command]
pub async fn tool_defaults(facade: State<'_, Arc<Facade>>) -> Result<ToolDefaults, String> {
    facade.tool_defaults().map_err(|err| err.to_string())
}

/// Replaces the Tools sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_tool_defaults(
    tools: ToolDefaults,
    facade: State<'_, Arc<Facade>>,
) -> Result<ToolDefaults, String> {
    facade
        .set_tool_defaults(tools)
        .map_err(|err| err.to_string())
}

/// The Integrations settings — the MCP and HTTP-API master toggles.
#[tauri::command]
pub async fn integration_settings(facade: State<'_, Arc<Facade>>) -> Result<Integrations, String> {
    facade.integration_settings().map_err(|err| err.to_string())
}

/// Replaces the Integrations sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_integration_settings(
    integrations: Integrations,
    facade: State<'_, Arc<Facade>>,
) -> Result<Integrations, String> {
    facade
        .set_integration_settings(integrations)
        .map_err(|err| err.to_string())
}

/// The MCP feature-group enablement — which feature-tool groups the server serves.
#[tauri::command]
pub async fn mcp_tool_groups(facade: State<'_, Arc<Facade>>) -> Result<McpToolGroups, String> {
    facade.mcp_tool_groups().map_err(|err| err.to_string())
}

/// Enables or disables one MCP feature group (auto-save), returning the updated enablement.
#[tauri::command]
pub async fn set_mcp_tool_group(
    group: McpFeatureGroup,
    enabled: bool,
    facade: State<'_, Arc<Facade>>,
) -> Result<McpToolGroups, String> {
    facade
        .set_mcp_tool_group(group, enabled)
        .map_err(|err| err.to_string())
}

/// The soloist-mcp helper binary's file name — resolved as a sibling of the app binary
/// when present, else assumed reachable on PATH.
const MCP_HELPER_BIN: &str = "soloist-mcp";

/// What a generated MCP client snippet needs: the helper command and the data-directory
/// facts. Presentation data for the Integrations panel, resolved app-side because the
/// binary's own location is an adapter concern, not domain state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct McpSetupInfo {
    /// The command a client should launch: the absolute path to the helper next to this
    /// binary when it exists there, else the bare name (PATH lookup).
    pub helper_path: String,
    /// The resolved data directory, for display beside the snippet.
    pub data_dir: String,
    /// Whether the data directory is overridden via the environment — when true, every
    /// snippet must carry the variable or the helper would miss the socket.
    pub data_dir_overridden: bool,
}

/// The helper command for a snippet, given this binary's own path: the sibling
/// `soloist-mcp` when it exists (a packaged or `cargo build` layout), else the bare name.
fn helper_command(exe: Option<std::path::PathBuf>) -> String {
    exe.as_deref()
        .and_then(std::path::Path::parent)
        .map(|dir| dir.join(MCP_HELPER_BIN))
        .filter(|sibling| sibling.exists())
        .map(|sibling| sibling.display().to_string())
        .unwrap_or_else(|| MCP_HELPER_BIN.to_owned())
}

/// The facts the Integrations panel renders MCP client snippets from.
#[tauri::command]
pub async fn mcp_setup_info() -> Result<McpSetupInfo, String> {
    let data_dir = soloist_ipc::data_dir().map_err(|err| err.to_string())?;
    Ok(McpSetupInfo {
        helper_path: helper_command(std::env::current_exe().ok()),
        data_dir: data_dir.display().to_string(),
        data_dir_overridden: soloist_ipc::data_dir_overridden(),
    })
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
