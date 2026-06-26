//! The per-project settings command surface: one thin wrapper per [`Facade`] per-project method.
//!
//! Each command marshals webview arguments, calls the one core method, and maps its typed error to
//! a string the UI renders. The setters auto-save (the core persists on every change) and return the
//! stored value; the shared-command edits route through the comment-preserving `solo.yml` write and
//! return the commands left needing trust; the move transfers a command between the shared and local
//! stores. The page command assembles the whole read model in one call. No policy lives here — the
//! per-project settings are the single source, driven identically by every front.

use std::sync::Arc;

use soloist_core::{
    Facade, ProcessSpec, ProjectId, ProjectSettings, ProjectSettingsPage, TrustReviewCommand,
};
use tauri::State;

/// The assembled per-project settings page — root, config validity, command roster, and live
/// counts — that the settings page renders directly.
#[tauri::command]
pub async fn project_settings_page(
    project: ProjectId,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettingsPage, String> {
    facade
        .project_settings_page(project)
        .map_err(|err| err.to_string())
}

/// One project's local settings, reading the documented defaults when nothing is stored yet.
#[tauri::command]
pub async fn project_settings(
    project: ProjectId,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .project_settings(project)
        .map_err(|err| err.to_string())
}

/// Engages or releases the project-level auto-start gate (auto-save), returning the updated settings.
#[tauri::command]
pub async fn set_project_auto_start_gate(
    project: ProjectId,
    engaged: bool,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .set_project_auto_start_gate(project, engaged)
        .map_err(|err| err.to_string())
}

/// Sets (or clears, with `null`) this project's editor override (auto-save).
#[tauri::command]
pub async fn set_project_editor_override(
    project: ProjectId,
    editor: Option<String>,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .set_project_editor_override(project, editor)
        .map_err(|err| err.to_string())
}

/// Toggles crash/exit alerts for this project (auto-save).
#[tauri::command]
pub async fn set_project_crash_exit_alerts(
    project: ProjectId,
    enabled: bool,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .set_project_crash_exit_alerts(project, enabled)
        .map_err(|err| err.to_string())
}

/// Toggles project-wide terminal alerts (auto-save).
#[tauri::command]
pub async fn set_project_terminal_alerts(
    project: ProjectId,
    enabled: bool,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .set_project_terminal_alerts(project, enabled)
        .map_err(|err| err.to_string())
}

/// Overrides one command's terminal alerts for this project (auto-save).
#[tauri::command]
pub async fn set_command_terminal_alerts(
    project: ProjectId,
    command: String,
    enabled: bool,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .set_command_terminal_alerts(project, &command, enabled)
        .map_err(|err| err.to_string())
}

/// Adds a command to the project's `solo.yml` (shared), returning the commands left needing trust.
#[tauri::command]
pub async fn add_shared_command(
    project: ProjectId,
    name: String,
    spec: ProcessSpec,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustReviewCommand>, String> {
    facade
        .add_shared_command(project, &name, spec)
        .map_err(|err| err.to_string())
}

/// Replaces a shared command's spec in `solo.yml`, returning the commands left needing re-trust.
#[tauri::command]
pub async fn edit_shared_command(
    project: ProjectId,
    name: String,
    spec: ProcessSpec,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustReviewCommand>, String> {
    facade
        .edit_shared_command(project, &name, spec)
        .map_err(|err| err.to_string())
}

/// Renames a shared command in `solo.yml` (a pure rename preserves trust).
#[tauri::command]
pub async fn rename_shared_command(
    project: ProjectId,
    from: String,
    to: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustReviewCommand>, String> {
    facade
        .rename_shared_command(project, &from, &to)
        .map_err(|err| err.to_string())
}

/// Removes a shared command from `solo.yml`.
#[tauri::command]
pub async fn remove_shared_command(
    project: ProjectId,
    name: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustReviewCommand>, String> {
    facade
        .remove_shared_command(project, &name)
        .map_err(|err| err.to_string())
}

/// Adds an app-local command (never written to `solo.yml`), returning the updated settings.
#[tauri::command]
pub async fn add_local_command(
    project: ProjectId,
    name: String,
    spec: ProcessSpec,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .add_local_command(project, &name, spec)
        .map_err(|err| err.to_string())
}

/// Replaces an app-local command's spec, keeping its position.
#[tauri::command]
pub async fn edit_local_command(
    project: ProjectId,
    name: String,
    spec: ProcessSpec,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .edit_local_command(project, &name, spec)
        .map_err(|err| err.to_string())
}

/// Renames an app-local command, keeping its position.
#[tauri::command]
pub async fn rename_local_command(
    project: ProjectId,
    from: String,
    to: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .rename_local_command(project, &from, &to)
        .map_err(|err| err.to_string())
}

/// Removes an app-local command.
#[tauri::command]
pub async fn remove_local_command(
    project: ProjectId,
    name: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .remove_local_command(project, &name)
        .map_err(|err| err.to_string())
}

/// Moves a shared command out of `solo.yml` into the app-local overlay ("Make local").
#[tauri::command]
pub async fn make_command_local(
    project: ProjectId,
    name: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectSettings, String> {
    facade
        .make_command_local(project, &name)
        .map_err(|err| err.to_string())
}

/// Moves an app-local command into `solo.yml` ("Save to solo.yml"), returning the commands left
/// needing trust.
#[tauri::command]
pub async fn save_command_to_yaml(
    project: ProjectId,
    name: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustReviewCommand>, String> {
    facade
        .save_command_to_yaml(project, &name)
        .map_err(|err| err.to_string())
}

/// Sets or clears (`null`) the project's `solo.yml` icon (shared). Rejects an `.svg` path.
#[tauri::command]
pub async fn set_project_icon(
    project: ProjectId,
    icon: Option<String>,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade
        .set_project_icon(project, icon)
        .map_err(|err| err.to_string())
}
