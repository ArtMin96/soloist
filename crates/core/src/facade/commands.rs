//! Project command editing (context C8): create, edit, rename, and delete a project's commands, and
//! move one between the shared `solo.yml` (C1) and the app-local overlay (per-project settings).
//!
//! A **shared** edit routes through the C1 config write path — a comment-preserving `solo.yml` write
//! that re-trusts the changed command (the returned [`TrustReviewCommand`]s are the ones now needing
//! trust). A **local** command touches only app state and is never written to `solo.yml`. The
//! **move** transfers a command between the two stores: it adds to the destination before removing
//! from the source (and rolls back on failure), so a command is never lost and the two stores never
//! both keep it after the move completes. One behaviour, many fronts.

use std::path::{Path, PathBuf};

use super::Facade;
use crate::config::{
    check_command, check_command_name, ConfigWriteError, InvalidCommand, ProcessSpec, SoloYml,
    TrustReviewCommand, WriteError,
};
use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::projects::ProjectError;
use crate::settings::ProjectSettings;

/// The image formats a project icon may use, matched against the path extension so an
/// unsupported file is rejected at the write rather than silently stored. The set mirrors the
/// [`ConfigWriteError::UnsupportedIcon`] message and the read-side image sniff.
const SUPPORTED_ICON_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "ico", "webp"];

fn is_supported_icon(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SUPPORTED_ICON_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

impl Facade {
    /// Sets or clears (`None`) the project's icon in `solo.yml` — a shared field. Rejects a path
    /// whose extension is not a supported raster format (png/jpg/jpeg/gif/ico/webp), matching the
    /// editor. An icon change is not a `processes:` edit, so the file is re-rendered rather than
    /// edited in place, and the project's display metadata is refreshed so the new icon shows
    /// without reopening the project.
    pub fn set_project_icon(
        &self,
        project: ProjectId,
        icon: Option<String>,
    ) -> Result<(), ConfigWriteError> {
        if let Some(path) = &icon {
            if !is_supported_icon(path) {
                return Err(ConfigWriteError::UnsupportedIcon(path.clone()));
            }
        }
        self.config.write(project, move |config| {
            config.icon = icon.map(PathBuf::from);
            Ok(())
        })?;
        self.refresh_project_metadata(project)
    }

    /// Re-registers a project's display metadata (name/icon) from its current `solo.yml` and
    /// announces the change with [`DomainEvent::ProjectOpened`], so the project read model (sidebar
    /// row, header) reflects a shared name/icon edit without reopening — an icon/name change is not
    /// a `processes:` diff, so the config write itself publishes nothing and the durable project
    /// record (which carries the displayed icon) would otherwise stay stale. A no-op when the
    /// project is not registered.
    fn refresh_project_metadata(&self, project: ProjectId) -> Result<(), ConfigWriteError> {
        let Some(record) = self.projects.get(project)? else {
            return Ok(());
        };
        let config = self.config.current(project);
        let name = config.as_ref().and_then(|c| c.name.as_deref());
        let icon = config.as_ref().and_then(|c| c.icon.as_deref());
        self.projects
            .add(&record.root, name, icon)
            .map_err(|err| match err {
                ProjectError::Store(source) => ConfigWriteError::Store(source),
                ProjectError::Root { source } => ConfigWriteError::Write(WriteError::Write {
                    path: record.root.clone(),
                    source,
                }),
            })?;
        self.bus.publish(DomainEvent::ProjectOpened { id: project });
        Ok(())
    }

    /// Whether a command of `name` already lives in the project's shared `solo.yml` (the last-synced
    /// snapshot). Paired with the local-overlay check, this keeps a command name unique across the
    /// two stores, so a name is never ambiguous between shared and local.
    fn shared_command_exists(&self, project: ProjectId, name: &str) -> bool {
        self.config
            .current(project)
            .is_some_and(|config| config.processes.contains_key(name))
    }

    /// Applies a **user-initiated** mutation to the project's `solo.yml` and then, when the project
    /// auto-trusts command changes, trusts whatever the write left needing trust. The one shared
    /// entry point for every command edit a user makes through Soloist, so the auto-trust policy is
    /// applied in exactly one place. An external `solo.yml` edit never reaches here — it syncs
    /// through [`ConfigEngine::sync`](crate::config::ConfigEngine::sync), which holds no settings
    /// and never trusts — so a change made outside Soloist still requires explicit trust.
    fn write_shared_command<F>(
        &self,
        project: ProjectId,
        mutate: F,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError>
    where
        F: FnOnce(&mut SoloYml) -> Result<(), ConfigWriteError>,
    {
        let pending = self.config.write(project, mutate)?;
        self.auto_trust_user_writes(project, pending)
    }

    /// Trusts the commands a user-initiated write left needing trust when this project's
    /// "automatically trust command changes" setting is on, returning the commands still needing
    /// trust (empty once auto-trust handled them); with the setting off it returns `pending`
    /// unchanged for the trust-review prompt. Trusting routes through the same path as an explicit
    /// grant ([`Self::trust_command`]) — the durable variant plus the live read model — so an
    /// auto-trusted command is startable at once.
    fn auto_trust_user_writes(
        &self,
        project: ProjectId,
        pending: Vec<TrustReviewCommand>,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        if pending.is_empty()
            || !self
                .project_settings
                .get(&project)?
                .auto_trust_command_changes
        {
            return Ok(pending);
        }
        for command in &pending {
            if let Some(spec) = self.config.spec(project, &command.name) {
                self.trust.trust(project, &spec)?;
                self.supervisor.mark_trusted(project, &spec.variant_hash());
            }
        }
        Ok(Vec::new())
    }

    /// Adds a command to the project's `solo.yml` (shared). Refused if the name is already taken by
    /// a local command, so the two stores never hold the same name. Returns the commands the write
    /// left needing trust (the new command, until trusted).
    pub fn add_shared_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        check_command(name, &spec)?;
        if self
            .project_settings
            .get(&project)?
            .local_commands
            .contains_key(name)
        {
            return Err(ConfigWriteError::DuplicateCommand(name.to_owned()));
        }
        self.insert_shared_command(project, name, spec)
    }

    /// The raw shared insert: writes the command to `solo.yml`, refusing only a same-name shared
    /// command. Used by [`add_shared_command`](Self::add_shared_command) (after its local-name
    /// guard) and by [`save_command_to_yaml`](Self::save_command_to_yaml) — the move adds the
    /// command while it still lives in the local overlay, so it must not run the cross-store guard.
    fn insert_shared_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        let name = name.to_owned();
        self.write_shared_command(project, move |config| {
            if config.processes.contains_key(&name) {
                return Err(ConfigWriteError::DuplicateCommand(name));
            }
            config.processes.insert(name, spec);
            Ok(())
        })
    }

    /// Replaces a shared command's spec in `solo.yml`, keeping its position. Returns the commands the
    /// edit left needing re-trust (the edited command, if its variant changed).
    pub fn edit_shared_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        check_command(name, &spec)?;
        let name = name.to_owned();
        self.write_shared_command(project, move |config| {
            if !config.processes.contains_key(&name) {
                return Err(ConfigWriteError::UnknownCommand);
            }
            config.processes.insert(name, spec);
            Ok(())
        })
    }

    /// Renames a shared command in `solo.yml`, keeping its position. A pure rename preserves trust
    /// (trust is keyed on the command variant, not the name). Refused if the new name is already
    /// taken by a local command, so the two stores never collide.
    pub fn rename_shared_command(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        check_command_name(to)?;
        if from != to
            && self
                .project_settings
                .get(&project)?
                .local_commands
                .contains_key(to)
        {
            return Err(ConfigWriteError::DuplicateCommand(to.to_owned()));
        }
        let (from, to) = (from.to_owned(), to.to_owned());
        self.write_shared_command(project, move |config| {
            if !config.processes.contains_key(&from) {
                return Err(ConfigWriteError::UnknownCommand);
            }
            if from != to && config.processes.contains_key(&to) {
                return Err(ConfigWriteError::DuplicateCommand(to));
            }
            if let Some(idx) = config.processes.get_index_of(&from) {
                if let Some((_, spec)) = config.processes.shift_remove_index(idx) {
                    config.processes.shift_insert(idx, to, spec);
                }
            }
            Ok(())
        })
    }

    /// Removes a shared command from `solo.yml`.
    pub fn remove_shared_command(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        let name = name.to_owned();
        self.write_shared_command(project, move |config| {
            config
                .processes
                .shift_remove(&name)
                .ok_or(ConfigWriteError::UnknownCommand)?;
            Ok(())
        })
    }

    /// Adds an app-local command (never written to `solo.yml`). Refused if the name is already taken
    /// by a shared command, so the two stores never hold the same name. Returns the updated settings.
    pub fn add_local_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<ProjectSettings, LocalCommandError> {
        check_command(name, &spec)?;
        if self.shared_command_exists(project, name) {
            return Err(LocalCommandError::Duplicate(name.to_owned()));
        }
        if self
            .project_settings
            .get(&project)?
            .local_commands
            .contains_key(name)
        {
            return Err(LocalCommandError::Duplicate(name.to_owned()));
        }
        let name = name.to_owned();
        Ok(self.project_settings.update(&project, |s| {
            s.local_commands.insert(name, spec);
        })?)
    }

    /// Replaces an app-local command's spec, keeping its position.
    pub fn edit_local_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<ProjectSettings, LocalCommandError> {
        check_command(name, &spec)?;
        if !self
            .project_settings
            .get(&project)?
            .local_commands
            .contains_key(name)
        {
            return Err(LocalCommandError::Unknown);
        }
        let name = name.to_owned();
        Ok(self.project_settings.update(&project, |s| {
            s.local_commands.insert(name, spec);
        })?)
    }

    /// Renames an app-local command, keeping its position. Refused if the new name is already taken
    /// by a shared command, so the two stores never collide.
    pub fn rename_local_command(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<ProjectSettings, LocalCommandError> {
        check_command_name(to)?;
        let local = self.project_settings.get(&project)?.local_commands;
        if !local.contains_key(from) {
            return Err(LocalCommandError::Unknown);
        }
        if from != to && (local.contains_key(to) || self.shared_command_exists(project, to)) {
            return Err(LocalCommandError::Duplicate(to.to_owned()));
        }
        let (from, to) = (from.to_owned(), to.to_owned());
        Ok(self.project_settings.update(&project, |s| {
            if let Some(idx) = s.local_commands.get_index_of(&from) {
                if let Some((_, spec)) = s.local_commands.shift_remove_index(idx) {
                    s.local_commands.shift_insert(idx, to, spec);
                }
            }
        })?)
    }

    /// Removes an app-local command.
    pub fn remove_local_command(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<ProjectSettings, LocalCommandError> {
        if !self
            .project_settings
            .get(&project)?
            .local_commands
            .contains_key(name)
        {
            return Err(LocalCommandError::Unknown);
        }
        let name = name.to_owned();
        Ok(self.project_settings.update(&project, |s| {
            s.local_commands.shift_remove(&name);
        })?)
    }

    /// Moves a shared command out of `solo.yml` into the app-local overlay ("Make local"). The
    /// command is added locally before being removed from `solo.yml`; if the `solo.yml` write fails
    /// the local add is rolled back, so the command is never lost or duplicated.
    pub fn make_command_local(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<ProjectSettings, MoveCommandError> {
        let spec = self
            .config
            .spec(project, name)
            .ok_or(MoveCommandError::NotShared)?;
        let added = self.project_settings.update(&project, {
            let name = name.to_owned();
            move |s| {
                s.local_commands.insert(name, spec);
            }
        })?;
        match self.remove_shared_command(project, name) {
            Ok(_) => Ok(added),
            Err(err) => {
                let _ = self.project_settings.update(&project, |s| {
                    s.local_commands.shift_remove(name);
                });
                Err(MoveCommandError::Config(err))
            }
        }
    }

    /// Moves an app-local command into `solo.yml` ("Save to solo.yml"). The command is written to
    /// `solo.yml` first (re-trusting it); if that fails the local command is left intact, and if
    /// clearing the local copy then fails the shared write is rolled back — so the command is never
    /// lost or left in both stores. Returns the commands the write left needing trust.
    pub fn save_command_to_yaml(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<Vec<TrustReviewCommand>, MoveCommandError> {
        let spec = self
            .project_settings
            .get(&project)?
            .local_commands
            .get(name)
            .cloned()
            .ok_or(MoveCommandError::NotLocal)?;
        let commands = self.insert_shared_command(project, name, spec)?;
        if let Err(err) = self.project_settings.update(&project, |s| {
            s.local_commands.shift_remove(name);
        }) {
            // The shared write succeeded but clearing the local copy failed; remove the shared
            // command again so it never lives in both stores (mirrors make_command_local).
            let _ = self.remove_shared_command(project, name);
            return Err(MoveCommandError::Store(err));
        }
        Ok(commands)
    }
}

/// Why an app-local command edit failed.
#[derive(Debug, thiserror::Error)]
pub enum LocalCommandError {
    /// A command with that name already exists in this project (in the local overlay or in
    /// `solo.yml`), so the name is unavailable for a local command.
    #[error("a command named {0:?} already exists")]
    Duplicate(String),
    /// No local command of that name exists.
    #[error("no such local command")]
    Unknown,
    /// The command the mutation would store has no name or nothing to run.
    #[error(transparent)]
    Invalid(#[from] InvalidCommand),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why moving a command between the shared and local stores failed.
#[derive(Debug, thiserror::Error)]
pub enum MoveCommandError {
    /// The command is not in `solo.yml`, so it cannot be made local.
    #[error("no such command in solo.yml")]
    NotShared,
    /// The command is not in the local overlay, so it cannot be saved to `solo.yml`.
    #[error("no such local command")]
    NotLocal,
    #[error(transparent)]
    Config(#[from] ConfigWriteError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
