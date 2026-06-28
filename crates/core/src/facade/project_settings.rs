//! Per-project local-settings actions (context C8 → settings): the durable, app-local preference
//! surface the per-project settings page drives through the one façade. Keyed by `ProjectId` over
//! the same settings base as the global preferences, stored apart from the project's shared
//! `solo.yml` config (C1) and never written to it. Each setter routes through the store's single
//! `update` write primitive, so the UI, CLI, and any other front drive the same record.

use std::collections::HashMap;

use super::Facade;
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::process::ProcStatus;
use crate::projects::{ConfigStatus, ProjectCommandView, ProjectSettingsPage, Visibility};
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

    /// The assembled per-project settings page — one read the settings page renders directly: the
    /// project's root, whether its `solo.yml` currently loads, the shared and app-local command
    /// roster (each with its live status and resolved terminal-alert state), the live running/total
    /// counts, the local settings, and the resolved editor. One assembly behind the façade, so every
    /// front renders the same page from the same source.
    pub fn project_settings_page(
        &self,
        project: ProjectId,
    ) -> Result<ProjectSettingsPage, StoreError> {
        let root = self
            .projects
            .get(project)?
            .ok_or_else(|| StoreError::Backend("no such project is open".into()))?
            .root;
        let config = match crate::config::load(&crate::config::config_path(&root)) {
            Ok(_) => ConfigStatus {
                valid: true,
                error: None,
            },
            Err(err) => ConfigStatus {
                valid: false,
                error: Some(err.to_string()),
            },
        };

        let settings = self.project_settings.get(&project)?;
        // The live status of each of this project's processes, keyed by its display label (the
        // command name), so a command's row reflects whether it is running.
        let statuses: HashMap<String, ProcStatus> = self
            .supervisor
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project)
            .map(|view| (view.label, view.status))
            .collect();

        let shared = self.config.current(project).unwrap_or_default().processes;
        let mut commands = Vec::with_capacity(shared.len() + settings.local_commands.len());
        for (name, spec) in &shared {
            commands.push(ProjectCommandView::new(
                name.clone(),
                spec,
                Visibility::Shared,
                settings.terminal_alerts_for(name),
                statuses.get(name).copied(),
            ));
        }
        for (name, spec) in &settings.local_commands {
            commands.push(ProjectCommandView::new(
                name.clone(),
                spec,
                Visibility::Local,
                settings.terminal_alerts_for(name),
                statuses.get(name).copied(),
            ));
        }

        let running = commands
            .iter()
            .filter(|command| command.status == Some(ProcStatus::Running))
            .count();
        let total = shared.len() + settings.local_commands.len();
        let resolved_editor = self.resolved_project_editor(project)?;

        Ok(ProjectSettingsPage {
            project,
            root: root.display().to_string(),
            config,
            running,
            total,
            settings,
            resolved_editor,
            commands,
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
