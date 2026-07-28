//! Per-project local-settings actions (context C8 → settings): the durable, app-local preference
//! surface the per-project settings page drives through the one façade. Keyed by `ProjectId` over
//! the same settings base as the global preferences, stored apart from the project's shared
//! `solo.yml` config (C1) and never written to it. Each setter routes through the store's single
//! `update` write primitive, so the UI, CLI, and any other front drive the same record.

use std::collections::HashMap;

use super::Facade;
use crate::ids::{ProjectId, TemplateId};
use crate::ports::StoreError;
use crate::process::ProcStatus;
use crate::projects::{ConfigStatus, ProjectCommandView, ProjectSettingsPage, Visibility};
use crate::settings::{NotificationLevel, ProjectSettings, TemplateDefaults};
use crate::template::TemplateKind;

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

    /// Enables or disables auto-trusting this project's user-saved command changes and persists it
    /// (auto-save), returning the updated settings. When on, a command the user creates or edits
    /// through Soloist is trusted on save; a `solo.yml` edit made outside Soloist still syncs in
    /// untrusted and requires explicit trust.
    pub fn set_project_auto_trust_command_changes(
        &self,
        project: ProjectId,
        enabled: bool,
    ) -> Result<ProjectSettings, StoreError> {
        self.project_settings
            .update(&project, |s| s.auto_trust_command_changes = enabled)
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

    /// Sets how much this project notifies and persists it.
    pub fn set_project_notification_level(
        &self,
        project: ProjectId,
        level: NotificationLevel,
    ) -> Result<ProjectSettings, StoreError> {
        self.project_settings
            .update(&project, |s| s.notification_level = level)
    }

    /// Overrides one command's notification level for this project and persists it, or drops the
    /// override with `None` so the command inherits the project again. The command is keyed by
    /// name; an override only ever tightens what the project already admits.
    pub fn set_command_notification_level(
        &self,
        project: ProjectId,
        command: &str,
        level: Option<NotificationLevel>,
    ) -> Result<ProjectSettings, StoreError> {
        let command = command.to_owned();
        self.project_settings.update(&project, |s| match level {
            Some(level) => {
                s.command_notification_levels.insert(command, level);
            }
            None => {
                s.command_notification_levels.remove(&command);
            }
        })
    }

    /// This project's default-template selection per kind — which of its own templates each
    /// free-form kind seeds a new document from. Read per call (never cached alongside the template
    /// list), so a change takes effect on the next creation. Absent settings read as the documented
    /// defaults (none selected).
    pub fn template_defaults(&self, project: ProjectId) -> Result<TemplateDefaults, StoreError> {
        Ok(self.project_settings.get(&project)?.template_defaults)
    }

    /// Selects (or clears, with `None`) this project's default template for `kind` and persists it,
    /// returning the updated selection. The template is one of this project's own — a global one
    /// resolves to nothing at seeding time — and [`TemplateKind::Prompt`] has no seed default, so a
    /// set for it is a no-op.
    pub fn set_default_template(
        &self,
        kind: TemplateKind,
        project: ProjectId,
        template: Option<TemplateId>,
    ) -> Result<TemplateDefaults, StoreError> {
        Ok(self
            .project_settings
            .update(&project, |s| s.template_defaults.set(kind, template))?
            .template_defaults)
    }

    /// The assembled per-project settings page — one read the settings page renders directly: the
    /// project's root, whether its `solo.yml` currently loads, the shared and app-local command
    /// roster (each with its live status and its own notification-level override), the live running/total
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
                &settings,
                statuses.get(name).copied(),
            ));
        }
        for (name, spec) in &settings.local_commands {
            commands.push(ProjectCommandView::new(
                name.clone(),
                spec,
                Visibility::Local,
                &settings,
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
