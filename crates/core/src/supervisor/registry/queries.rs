//! Project-scoped read queries over the registry: the process sets the bulk command and
//! lifecycle operations select within a project. Split from the registry root to keep each
//! file single-purpose; as a child module these still reach the registry's private map.

use crate::ids::{ProcessId, ProjectId};
use crate::process::ProcStatus;
use crate::sync::lock;

use super::{Candidate, Registry};

impl Registry {
    /// Active processes within `project` — the targets of `stop_all`.
    pub(crate) fn live_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.view.project == project && entry.view.status.is_active())
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Every process within `project`, whatever its kind or status — the targets of
    /// `close_all`, which forgets the whole project.
    pub(crate) fn ids_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.view.project == project)
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Running processes within `project` — the targets of `restart_running`.
    pub(crate) fn running_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.project == project && entry.view.status == ProcStatus::Running
            })
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Active `Command` processes within `project` — the targets of `stop_all_commands`
    /// (which, unlike `stop_all`, leaves agents and terminals running).
    pub(crate) fn live_commands_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.view.is_command_in(project) && entry.view.status.is_active())
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Every `Command` within `project`, whatever its status — the targets of
    /// `restart_all_commands`, which restarts the running ones and starts the resting ones.
    pub(crate) fn commands_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.view.is_command_in(project))
            .map(|entry| entry.view.id)
            .collect()
    }

    /// The `Command` in `project` whose display label is `name`, if any — how config-reload
    /// resolves a `solo.yml` process name to the registration to update, rename, or drop.
    /// `solo.yml` names are unique per project, so at most one command matches; terminals and
    /// agents are excluded (a launched process is never a config command).
    pub(crate) fn command_id_by_name(&self, project: ProjectId, name: &str) -> Option<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .find(|entry| entry.view.is_command_in(project) && entry.view.label == name)
            .map(|entry| entry.view.id)
    }

    /// Stopped, `auto_start` commands within `project` — the candidates `start_all`
    /// trust-checks before launching.
    pub(crate) fn auto_start_candidates(&self, project: ProjectId) -> Vec<Candidate> {
        self.stopped_command_candidates(project, true)
    }

    /// Stopped commands within `project`, regardless of `auto_start` — the candidates
    /// `start_all_commands` trust-checks before launching. Distinct from
    /// `auto_start_candidates`, which the auto-start path narrows to `auto_start` commands.
    pub(crate) fn command_candidates(&self, project: ProjectId) -> Vec<Candidate> {
        self.stopped_command_candidates(project, false)
    }

    /// Stopped commands within `project`, optionally narrowed to `auto_start` ones — the
    /// shared query behind both bulk-start candidate sets, so they never drift in how a
    /// candidate is shaped.
    fn stopped_command_candidates(
        &self,
        project: ProjectId,
        auto_start_only: bool,
    ) -> Vec<Candidate> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.is_command_in(project)
                    && entry.view.status == ProcStatus::Stopped
                    && (!auto_start_only || entry.auto_start)
            })
            .map(|entry| Candidate {
                id: entry.view.id,
                trust_variant: entry.trust_variant,
                launch: entry.launch.clone(),
            })
            .collect()
    }
}
