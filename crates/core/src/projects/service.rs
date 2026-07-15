//! The project lifecycle: opening and restoring projects.
//!
//! [`ProjectService`] is the one place the "open a project" behaviour lives. It
//! orchestrates the contexts a project open touches — the registry (durable identity),
//! the `solo.yml` config engine, the supervisor (command registration, orphan
//! reconciliation, auto-start), and the event bus — so no consumer (the [`Facade`], an
//! adapter, the UI) re-implements that sequence. Consumers call [`ProjectService::open`]
//! or [`ProjectService::restore`]; they do not decide how a project opens.
//!
//! [`Facade`]: crate::facade::Facade

use std::path::Path;

use crate::config::{ConfigEngine, ConfigError, SoloYml, SyncError, WriteError};
use crate::configchange::ConfigSync;
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::ports::{ProjectRecord, StoreError};
use crate::projects::{ProjectError, Projects};
use crate::supervision::run_blocking;
use crate::supervisor::{Registration, Supervisor, SupervisorError};

/// The project lifecycle service. Borrows the contexts a project open spans — assembled
/// once by the composition root (the [`Facade`](crate::facade::Facade) owns them) — so the
/// orchestration is defined here and nowhere else.
pub struct ProjectService<'a> {
    projects: &'a Projects,
    config: &'a ConfigEngine,
    supervisor: &'a Supervisor,
    bus: &'a EventBus,
}

impl<'a> ProjectService<'a> {
    /// Assembles the service over the contexts it orchestrates.
    pub fn new(
        projects: &'a Projects,
        config: &'a ConfigEngine,
        supervisor: &'a Supervisor,
        bus: &'a EventBus,
    ) -> Self {
        Self {
            projects,
            config,
            supervisor,
            bus,
        }
    }

    /// Opens a project: registers its root (assigning the durable [`ProjectId`]), loads
    /// its `solo.yml`, registers each declared process as a trust-gated command, then
    /// reconciles leftover process groups and starts the trusted auto-start commands.
    ///
    /// When the folder has no `solo.yml`, one is auto-created from its detected commands
    /// before opening, so a project opened from an arbitrary folder is usable; an existing
    /// `solo.yml` is never rewritten. Reconciliation runs **after** registration so a
    /// leftover group matching a `solo.yml` command is adopted rather than mis-surfaced as
    /// an orphan. Starting is the supervisor's trusted-auto-start subset, so a detected
    /// (hence untrusted) command is registered (visible, `Stopped`) but never run until its
    /// variant is trusted. Returns the project's id, how many processes its `solo.yml`
    /// declared, and whether the file was just created, so the caller can tell the user
    /// what happened instead of doing so silently. Must run within a `tokio` runtime
    /// (reconciliation and starting do).
    pub fn open(&self, root: &Path) -> Result<ProjectLoad, LoadProjectError> {
        let (record, config, created) = self.open_and_register(root)?;
        self.supervisor.reconcile_orphans();
        self.supervisor.start_all(record.id)?;
        Ok(ProjectLoad {
            id: record.id,
            processes: config.processes.len(),
            created,
        })
    }

    /// Re-registers every known project without starting anything (session restore on
    /// launch), then reconciles orphans once. Each project's commands reappear **resting**,
    /// so the sidebar shows your projects across runs while nothing is spawned on startup —
    /// starting stays an explicit action. Best-effort: a project whose root or `solo.yml`
    /// is no longer readable is skipped, never failing the launch. Must run within a
    /// `tokio` runtime (reconciliation may adopt a leftover group).
    pub fn restore(&self) {
        let Ok(projects) = self.projects.list() else {
            return;
        };
        for project in projects {
            let _ = self.open_and_register(&project.root);
        }
        self.supervisor.reconcile_orphans();
    }

    /// Reloads a project's `solo.yml` and reconciles the supervisor's command registrations to
    /// it — the counterpart to [`Self::open`] for a project already open. Re-reads the file via
    /// the config engine (which also announces [`DomainEvent::ConfigChanged`]), then applies the
    /// change set to the registry: an **added** command is registered resting (untrusted until
    /// the user trusts its variant, like [`Self::restore`] — reload never starts anything); an
    /// **updated** command's spec is replaced in place, keeping its id (so a reload never
    /// duplicates a command) and never killing a running process (the new spec takes effect on
    /// its next restart, which the trust gate re-checks); a **renamed** command's label is
    /// updated in place, preserving trust (keyed on the variant) and the id; a **removed**
    /// command is dropped only if it is resting — a running one is left untouched. Returns the
    /// applied change set, or `None` when the file is byte-identical (a reload is then a no-op).
    /// Must run within a `tokio` runtime (registering announces on the bus).
    pub fn reload(&self, project: ProjectId) -> Result<Option<ConfigSync>, ReloadError> {
        // Resolve the root first so an unknown project is a clear error, not a silent no-op:
        // `config.sync` returns `None` for both an unchanged file and a project it never opened.
        let root = self
            .projects
            .get(project)?
            .ok_or(ReloadError::UnknownProject)?
            .root;
        let Some(diff) = self.config.sync(project)? else {
            return Ok(None);
        };
        // `sync` just refreshed and stored the parsed config, so `current` is `Some` here; the
        // guard keeps the reconcile all-or-nothing rather than defaulting to an empty config.
        let Some(config) = self.config.current(project) else {
            return Ok(Some(diff));
        };

        for name in &diff.added {
            if let Some(spec) = config.processes.get(name) {
                let registration = Registration::command(project, &root, name, spec);
                // A command re-added after being removed while running is still registered (a
                // running command is kept on removal), so reconcile it in place rather than
                // minting a second registration under the same name.
                match self.supervisor.command_id_by_name(project, name) {
                    Some(id) => {
                        self.supervisor.update_command(id, registration);
                    }
                    None => {
                        self.supervisor.register(registration);
                    }
                }
            }
        }
        for name in &diff.updated {
            if let (Some(id), Some(spec)) = (
                self.supervisor.command_id_by_name(project, name),
                config.processes.get(name),
            ) {
                self.supervisor
                    .update_command(id, Registration::command(project, &root, name, spec));
            }
        }
        for rename in &diff.renamed {
            if let (Some(id), Some(spec)) = (
                self.supervisor.command_id_by_name(project, &rename.from),
                config.processes.get(&rename.to),
            ) {
                self.supervisor
                    .update_command(id, Registration::command(project, &root, &rename.to, spec));
            }
        }
        for name in &diff.removed {
            if let Some(id) = self.supervisor.command_id_by_name(project, name) {
                self.supervisor.deregister_if_resting(id);
            }
        }
        Ok(Some(diff))
    }

    /// Removes a project — the counterpart to [`Self::open`]. Closes every one of its
    /// processes first (each live group is stopped and reaped **before** anything is
    /// forgotten, so no child is abandoned), evicts its `solo.yml` sync state, deletes its
    /// durable record — the store cascades to all of its project-scoped state (trust,
    /// leases, timers, scratchpads, todos, key-value, settings, project prompt templates)
    /// — and announces [`DomainEvent::ProjectRemoved`]. Files on disk are never touched:
    /// the folder and its `solo.yml` remain, so re-opening the folder later starts fresh
    /// (and untrusted). Must run within a `tokio` runtime (closing awaits each actor's
    /// exit).
    pub async fn remove(&self, project: ProjectId) -> Result<(), RemoveProjectError> {
        // Both store calls are synchronous, so each runs off the runtime worker: this is an async
        // path, and the delete is the widest write the app makes — the record cascades to every
        // project-scoped table — so a slow or full disk must not park a worker on its `fsync`.
        let repo = self.projects.repo();
        if run_blocking(move || repo.get(project)).await?.is_none() {
            return Err(RemoveProjectError::UnknownProject);
        }
        self.supervisor.close_all(project).await;
        self.config.forget(project);
        let repo = self.projects.repo();
        run_blocking(move || repo.remove(project)).await?;
        self.bus
            .publish(DomainEvent::ProjectRemoved { id: project });
        Ok(())
    }

    /// Adds the project (auto-creating its `solo.yml` when absent), loads the config,
    /// persists the resolved display metadata, announces the open, and registers each
    /// command as a trust-gated process — the shared path under [`Self::open`] (which then
    /// reconciles and starts) and [`Self::restore`] (which does neither). Returns the
    /// durable record, the parsed config, and whether the `solo.yml` was just created. Does
    /// not reconcile orphans or start — the caller decides.
    fn open_and_register(
        &self,
        root: &Path,
    ) -> Result<(ProjectRecord, SoloYml, bool), LoadProjectError> {
        let record = self.projects.add(root, None, None)?;
        let created = crate::config::create_if_absent(&record.root)?;
        let config = self.config.open(record.id, record.root.clone())?;
        // Persist the project's display metadata now the config is known. The id had to
        // be assigned first (`config.open` needs it), but the `name`/`icon` come from the
        // file — so a second idempotent upsert (keyed on the canonical root) records them.
        let record =
            self.projects
                .add(&record.root, config.name.as_deref(), config.icon.as_deref())?;
        // Announce the project before its processes, so an adapter re-reading the project
        // read model has it in view before any `ProcessSpawned` references it.
        self.bus
            .publish(DomainEvent::ProjectOpened { id: record.id });
        for (name, spec) in &config.processes {
            let registration = Registration::command(record.id, &record.root, name, spec);
            // Opening is idempotent per command: re-opening an already-open project (the folder
            // picker on the same folder, a second `soloist <path>` forwarded by single-instance)
            // reconciles each command in place — the reload rule — rather than minting a second
            // registration under the same name.
            match self.supervisor.command_id_by_name(record.id, name) {
                Some(id) => {
                    self.supervisor.update_command(id, registration);
                }
                None => {
                    self.supervisor.register(registration);
                }
            }
        }
        Ok((record, config, created))
    }
}

/// The outcome of opening a project: its durable id, how many processes its `solo.yml`
/// declared, and whether that `solo.yml` was just auto-created from detected commands.
/// `created` lets the caller tell the user a config was made for them; `processes == 0`
/// with `created == false` means an existing `solo.yml` declared nothing — either way the
/// caller surfaces it so opening a project is never silent.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ProjectLoad {
    pub id: ProjectId,
    pub processes: usize,
    pub created: bool,
}

/// Why opening a project failed: resolving/persisting its root, reading its `solo.yml`,
/// auto-creating one, or starting its trusted commands.
#[derive(Debug, thiserror::Error)]
pub enum LoadProjectError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Write(#[from] WriteError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

/// Why reloading a project's `solo.yml` failed: the project is not open, or re-reading the
/// file / a durable read while resolving its root failed.
#[derive(Debug, thiserror::Error)]
pub enum ReloadError {
    #[error("no such project is open")]
    UnknownProject,
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why removing a project failed: it is not registered, or the durable read/delete failed.
/// Its processes are only closed once the id resolves, so an unknown id removes nothing.
#[derive(Debug, thiserror::Error)]
pub enum RemoveProjectError {
    #[error("no such project is open")]
    UnknownProject,
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
