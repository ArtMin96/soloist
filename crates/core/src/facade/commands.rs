//! Project command editing (context C8): create, edit, rename, and delete a project's commands, and
//! move one between the shared `solo.yml` (C1) and the app-local overlay (per-project settings).
//!
//! A **shared** edit routes through the C1 config write path — a comment-preserving `solo.yml` write
//! that re-trusts the changed command (the returned [`TrustReviewCommand`]s are the ones now needing
//! trust). A **local** command touches only app state and is never written to `solo.yml`. The
//! **move** transfers a command between the two stores: it adds to the destination before removing
//! from the source (and rolls back on failure), so a command is never lost and the two stores never
//! both keep it after the move completes. One behaviour, many fronts.

use super::Facade;
use crate::config::{ConfigWriteError, ProcessSpec, TrustReviewCommand};
use crate::ids::ProjectId;
use crate::ports::StoreError;
use crate::settings::ProjectSettings;

impl Facade {
    /// Adds a command to the project's `solo.yml` (shared). Returns the commands the write left
    /// needing trust (the new command, until trusted).
    pub fn add_shared_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        let name = name.to_owned();
        self.config.write(project, move |config| {
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
        let name = name.to_owned();
        self.config.write(project, move |config| {
            if !config.processes.contains_key(&name) {
                return Err(ConfigWriteError::UnknownCommand);
            }
            config.processes.insert(name, spec);
            Ok(())
        })
    }

    /// Renames a shared command in `solo.yml`, keeping its position. A pure rename preserves trust
    /// (trust is keyed on the command variant, not the name).
    pub fn rename_shared_command(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError> {
        let (from, to) = (from.to_owned(), to.to_owned());
        self.config.write(project, move |config| {
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
        self.config.write(project, move |config| {
            config
                .processes
                .shift_remove(&name)
                .ok_or(ConfigWriteError::UnknownCommand)?;
            Ok(())
        })
    }

    /// Adds an app-local command (never written to `solo.yml`). Returns the updated settings.
    pub fn add_local_command(
        &self,
        project: ProjectId,
        name: &str,
        spec: ProcessSpec,
    ) -> Result<ProjectSettings, LocalCommandError> {
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

    /// Renames an app-local command, keeping its position.
    pub fn rename_local_command(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<ProjectSettings, LocalCommandError> {
        let local = self.project_settings.get(&project)?.local_commands;
        if !local.contains_key(from) {
            return Err(LocalCommandError::Unknown);
        }
        if from != to && local.contains_key(to) {
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
    /// `solo.yml` first (re-trusting it); if that fails the local command is left intact. Returns the
    /// commands the write left needing trust.
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
        let commands = self.add_shared_command(project, name, spec)?;
        self.project_settings.update(&project, |s| {
            s.local_commands.shift_remove(name);
        })?;
        Ok(commands)
    }
}

/// Why an app-local command edit failed.
#[derive(Debug, thiserror::Error)]
pub enum LocalCommandError {
    /// A local command with that name already exists.
    #[error("a local command named {0:?} already exists")]
    Duplicate(String),
    /// No local command of that name exists.
    #[error("no such local command")]
    Unknown,
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
