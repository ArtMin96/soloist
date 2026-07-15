//! The durable-settings command surface: one thin wrapper per [`Facade`] settings method.
//!
//! Each command marshals webview arguments, calls the one core method, and maps the typed
//! [`StoreError`](soloist_core::StoreError) to a string the UI renders. The whole-tab setters
//! auto-save (the core persists the document on every change) and return the stored value, so
//! the frontend reflects exactly what was written. No policy lives here — the settings store
//! is the single source, driven identically by every front. Each store call runs on the blocking
//! pool via [`Facade::blocking`], so a settings write's `fsync` never parks a runtime worker.

use std::sync::Arc;

use soloist_core::{
    Appearance, Binding, Facade, HotkeyAction, HotkeyBindingView, Integrations, McpFeatureGroup,
    McpToolGroups, Notifications, Sidebar, ToolDefaults,
};
use tauri::State;

use crate::companion_bins::{self, is_executable_file, MCP_HELPER_BIN};
use crate::integration_servers::IntegrationServers;

/// The Appearance settings — theme and terminal typography.
#[tauri::command]
pub async fn appearance(facade: State<'_, Arc<Facade>>) -> Result<Appearance, String> {
    facade
        .blocking(|f| f.appearance())
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the Appearance sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_appearance(
    appearance: Appearance,
    facade: State<'_, Arc<Facade>>,
) -> Result<Appearance, String> {
    facade
        .blocking(move |f| f.set_appearance(appearance))
        .await
        .map_err(|err| err.to_string())
}

/// The Sidebar settings — what the process-tree sidebar shows.
#[tauri::command]
pub async fn sidebar_settings(facade: State<'_, Arc<Facade>>) -> Result<Sidebar, String> {
    facade
        .blocking(|f| f.sidebar_settings())
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the Sidebar sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_sidebar_settings(
    sidebar: Sidebar,
    facade: State<'_, Arc<Facade>>,
) -> Result<Sidebar, String> {
    facade
        .blocking(move |f| f.set_sidebar_settings(sidebar))
        .await
        .map_err(|err| err.to_string())
}

/// The hotkey keymap read model — every action with its scope, effective binding, and whether
/// it is still the code default.
#[tauri::command]
pub async fn hotkeys(facade: State<'_, Arc<Facade>>) -> Result<Vec<HotkeyBindingView>, String> {
    facade
        .blocking(|f| f.hotkeys())
        .await
        .map_err(|err| err.to_string())
}

/// Remaps one action to a new chord (auto-save), returning the updated keymap.
#[tauri::command]
pub async fn remap_hotkey(
    action: HotkeyAction,
    binding: Binding,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade
        .blocking(move |f| f.remap_hotkey(action, binding))
        .await
        .map_err(|err| err.to_string())
}

/// Disables one action — hover-and-press-x — until it is reset (auto-save).
#[tauri::command]
pub async fn disable_hotkey(
    action: HotkeyAction,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade
        .blocking(move |f| f.disable_hotkey(action))
        .await
        .map_err(|err| err.to_string())
}

/// Resets one action to its code default by dropping its override (auto-save).
#[tauri::command]
pub async fn reset_hotkey(
    action: HotkeyAction,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade
        .blocking(move |f| f.reset_hotkey(action))
        .await
        .map_err(|err| err.to_string())
}

/// Resets every action to its code default — "Reset all to defaults" (auto-save).
#[tauri::command]
pub async fn reset_all_hotkeys(
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<HotkeyBindingView>, String> {
    facade
        .blocking(|f| f.reset_all_hotkeys())
        .await
        .map_err(|err| err.to_string())
}

/// The Tools settings — the default editor and terminal.
#[tauri::command]
pub async fn tool_defaults(facade: State<'_, Arc<Facade>>) -> Result<ToolDefaults, String> {
    facade
        .blocking(|f| f.tool_defaults())
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the Tools sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_tool_defaults(
    tools: ToolDefaults,
    facade: State<'_, Arc<Facade>>,
) -> Result<ToolDefaults, String> {
    facade
        .blocking(move |f| f.set_tool_defaults(tools))
        .await
        .map_err(|err| err.to_string())
}

/// The Integrations settings — the MCP and HTTP-API master toggles.
#[tauri::command]
pub async fn integration_settings(facade: State<'_, Arc<Facade>>) -> Result<Integrations, String> {
    facade
        .blocking(|f| f.integration_settings())
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the Integrations sub-document (auto-save) and applies it to the live sockets, so a
/// change to either master toggle starts or stops its server immediately — no app restart —
/// then returns the stored value.
#[tauri::command]
pub async fn set_integration_settings(
    integrations: Integrations,
    facade: State<'_, Arc<Facade>>,
    servers: State<'_, Arc<IntegrationServers>>,
) -> Result<Integrations, String> {
    let stored = facade
        .blocking(move |f| f.set_integration_settings(integrations))
        .await
        .map_err(|err| err.to_string())?;
    servers.apply(stored).await;
    Ok(stored)
}

/// The Notifications settings — the master on/off for every desktop toast.
#[tauri::command]
pub async fn notification_settings(
    facade: State<'_, Arc<Facade>>,
) -> Result<Notifications, String> {
    facade
        .blocking(|f| f.notification_settings())
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the Notifications sub-document (auto-save), returning the stored value.
#[tauri::command]
pub async fn set_notification_settings(
    notifications: Notifications,
    facade: State<'_, Arc<Facade>>,
) -> Result<Notifications, String> {
    facade
        .blocking(move |f| f.set_notification_settings(notifications))
        .await
        .map_err(|err| err.to_string())
}

/// The MCP feature-group enablement — which feature-tool groups the server serves.
#[tauri::command]
pub async fn mcp_tool_groups(facade: State<'_, Arc<Facade>>) -> Result<McpToolGroups, String> {
    facade
        .blocking(|f| f.mcp_tool_groups())
        .await
        .map_err(|err| err.to_string())
}

/// Enables or disables one MCP feature group (auto-save), returning the updated enablement.
#[tauri::command]
pub async fn set_mcp_tool_group(
    group: McpFeatureGroup,
    enabled: bool,
    facade: State<'_, Arc<Facade>>,
) -> Result<McpToolGroups, String> {
    facade
        .blocking(move |f| f.set_mcp_tool_group(group, enabled))
        .await
        .map_err(|err| err.to_string())
}

/// What a generated MCP client snippet needs: the helper command and the data-directory
/// facts. Presentation data for the Integrations panel, resolved app-side because the
/// binary's own location is an adapter concern, not domain state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct McpSetupInfo {
    /// The command a client should launch: the stable copy exported into the data
    /// directory when present, else the absolute path to the helper next to this binary,
    /// else the bare name (PATH lookup).
    pub helper_path: String,
    /// The resolved data directory, for display beside the snippet.
    pub data_dir: String,
    /// Whether the data directory is overridden via the environment — when true, every
    /// snippet must carry the variable or the helper would miss the socket.
    pub data_dir_overridden: bool,
}

/// The helper command for a snippet: the copy exported into the data directory when an
/// executable file sits there (the one path that survives an AppImage's per-launch mount
/// and package upgrades), else the executable sibling of this binary (a packaged or
/// `cargo build` layout), else the bare name.
fn helper_command(exe: Option<std::path::PathBuf>, data_dir: &std::path::Path) -> String {
    let exported = companion_bins::exported_path(data_dir, MCP_HELPER_BIN);
    if is_executable_file(&exported) {
        return exported.display().to_string();
    }
    exe.as_deref()
        .and_then(std::path::Path::parent)
        .map(|dir| dir.join(MCP_HELPER_BIN))
        .filter(|sibling| is_executable_file(sibling))
        .map(|sibling| sibling.display().to_string())
        .unwrap_or_else(|| MCP_HELPER_BIN.to_owned())
}

/// The facts the Integrations panel renders MCP client snippets from.
#[tauri::command]
pub async fn mcp_setup_info() -> Result<McpSetupInfo, String> {
    let data_dir = soloist_ipc::data_dir().map_err(|err| err.to_string())?;
    Ok(McpSetupInfo {
        helper_path: helper_command(std::env::current_exe().ok(), &data_dir),
        data_dir: data_dir.display().to_string(),
        data_dir_overridden: soloist_ipc::data_dir_overridden(),
    })
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
