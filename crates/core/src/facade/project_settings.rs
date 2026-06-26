//! Per-project local-settings actions (context C8 → settings): the durable, app-local preference
//! surface the per-project settings page drives through the one façade. Keyed by `ProjectId` over
//! the same settings base as the global preferences, stored apart from the project's shared
//! `solo.yml` config (C1) and never written to it. Each setter routes through the store's single
//! `update` write primitive, so the UI, CLI, and any other front drive the same record.

use super::Facade;
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::settings::ProjectSettings;

impl Facade {
    /// One project's local settings. Absent settings read as the documented defaults (auto-start
    /// gate off; alerts on).
    pub fn project_settings(&self, project: ProjectId) -> Result<ProjectSettings, StoreError> {
        self.project_settings.get(&project)
    }

    /// Engages or releases the project-level auto-start gate and persists it (auto-save), returning
    /// the updated settings. Engaging it suppresses auto-start for the whole project on open.
    pub fn set_project_auto_start_gate(
        &self,
        project: ProjectId,
        engaged: bool,
    ) -> Result<ProjectSettings, StoreError> {
        self.project_settings
            .update(&project, |s| s.auto_start_gate = engaged)
    }

    /// Sets (or clears, with `None`) this project's editor override and persists it. A cleared
    /// override falls back to the global Tools default (see [`Self::resolved_project_editor`]).
    pub fn set_project_editor_override(
        &self,
        project: ProjectId,
        editor: Option<String>,
    ) -> Result<ProjectSettings, StoreError> {
        self.project_settings
            .update(&project, |s| s.editor_override = editor)
    }

    /// Toggles crash/exit alerts for this project and persists it.
    pub fn set_project_crash_exit_alerts(
        &self,
        project: ProjectId,
        enabled: bool,
    ) -> Result<ProjectSettings, StoreError> {
        self.project_settings
            .update(&project, |s| s.crash_exit_alerts = enabled)
    }

    /// Toggles project-wide terminal (bell/attention) alerts and persists it.
    pub fn set_project_terminal_alerts(
        &self,
        project: ProjectId,
        enabled: bool,
    ) -> Result<ProjectSettings, StoreError> {
        self.project_settings
            .update(&project, |s| s.terminal_alerts = enabled)
    }

    /// Overrides one command's terminal alerts for this project and persists it. The command is
    /// keyed by name; an unoverridden command follows the project-wide default.
    pub fn set_command_terminal_alerts(
        &self,
        project: ProjectId,
        command: &str,
        enabled: bool,
    ) -> Result<ProjectSettings, StoreError> {
        let command = command.to_owned();
        self.project_settings.update(&project, |s| {
            s.command_terminal_alerts.insert(command, enabled);
        })
    }

    /// The editor to open this project with — the project override, else the global Tools default
    /// (`None` = the system default). One resolver behind the façade, so every front resolves
    /// "which editor" identically (single source).
    pub fn resolved_project_editor(
        &self,
        project: ProjectId,
    ) -> Result<Option<String>, StoreError> {
        let settings = self.project_settings.get(&project)?;
        let global = self.settings.get(&())?.tools;
        Ok(settings.resolved_editor(&global).map(str::to_owned))
    }
}

#[cfg(test)]
#[path = "project_settings_tests.rs"]
mod tests;
