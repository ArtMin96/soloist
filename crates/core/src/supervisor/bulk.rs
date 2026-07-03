//! Project-wide bulk lifecycle operations: starting, stopping, and restarting every
//! applicable process in a project in one call. Each delegates to the per-process
//! lifecycle and trust gate, so bulk behaviour can never diverge from the single-process
//! paths.
//!
//! Two start scopes exist deliberately: [`Supervisor::start_all`] starts only `auto_start`
//! commands (the dashboard's launch-the-stack action and project auto-start), while
//! [`Supervisor::start_all_commands`] starts every trusted command (the MCP bulk tool).
//! Solo separates these as `start-auto` versus `start-all`.

use serde::{Deserialize, Serialize};

use crate::ids::{ProcessId, ProjectId};

use super::registry::Candidate;
use super::{Supervisor, SupervisorError};

/// The outcome of a bulk start: what was started, and what was skipped because its
/// command variant is not trusted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSummary {
    pub started: Vec<ProcessId>,
    pub skipped_untrusted: Vec<ProcessId>,
}

impl Supervisor {
    /// Starts every trusted `auto_start` command in a project; untrusted candidates are
    /// reported, not run. The dashboard's "start the stack" and project auto-start path.
    pub fn start_all(&self, project: ProjectId) -> Result<StartSummary, SupervisorError> {
        self.launch_all(project, self.registry.auto_start_candidates(project))
    }

    /// Starts every trusted command in a project regardless of `auto_start`; untrusted
    /// candidates are reported, not run. The MCP `start_all_commands` tool, distinct from
    /// the auto-start-only [`start_all`](Self::start_all).
    pub fn start_all_commands(&self, project: ProjectId) -> Result<StartSummary, SupervisorError> {
        self.launch_all(project, self.registry.command_candidates(project))
    }

    /// Trust-checks each candidate and launches the trusted ones, reporting the rest as
    /// skipped — the one launch loop both bulk-start scopes share, so they cannot diverge.
    fn launch_all(
        &self,
        project: ProjectId,
        candidates: Vec<Candidate>,
    ) -> Result<StartSummary, SupervisorError> {
        let mut summary = StartSummary::default();
        for candidate in candidates {
            let trusted = match &candidate.trust_variant {
                Some(variant) => self.trust.is_trusted(project, variant)?,
                None => true,
            };
            if trusted {
                if self.launch_actor(candidate.id, candidate.launch, None) {
                    summary.started.push(candidate.id);
                }
            } else {
                summary.skipped_untrusted.push(candidate.id);
            }
        }
        Ok(summary)
    }

    /// Requests a graceful stop of every live process in a project.
    pub fn stop_all(&self, project: ProjectId) {
        for id in self.registry.live_in(project) {
            self.stop(id);
        }
    }

    /// Requests a graceful stop of every running command in a project, returning how many
    /// were messaged. Unlike [`stop_all`](Self::stop_all) this leaves agents and terminals
    /// running — the MCP `stop_all_commands` tool acts on commands only.
    pub fn stop_all_commands(&self, project: ProjectId) -> usize {
        self.registry
            .live_commands_in(project)
            .into_iter()
            .filter(|&id| self.stop(id))
            .count()
    }

    /// Closes every process in a project — stopping and reaping each live group, then
    /// forgetting the entry (each announcing [`DomainEvent::ProcessRemoved`]) — so after it
    /// returns the project holds no registrations and no children. The project-removal
    /// teardown. Reuses the per-process [`close`](Supervisor::close), so bulk teardown can
    /// never diverge from the single-process path.
    ///
    /// [`DomainEvent::ProcessRemoved`]: crate::events::DomainEvent::ProcessRemoved
    pub async fn close_all(&self, project: ProjectId) {
        // Message every live process to stop up front so their termination grace windows
        // overlap; the per-process closes then await exits already under way. Bulk teardown
        // stays O(grace), not O(n × grace).
        self.stop_all(project);
        for id in self.registry.ids_in(project) {
            // `close` fails only on `NotFound` — a process closed concurrently — which a
            // bulk teardown tolerates by design.
            let _ = self.close(id).await;
        }
    }

    /// Restarts every currently-running process in a project (trusted only; an
    /// untrusted one is skipped).
    pub fn restart_running(&self, project: ProjectId) -> Result<(), SupervisorError> {
        self.restart_each(self.registry.running_in(project))
    }

    /// Restarts every trusted command in a project, bringing the whole command set up
    /// fresh: a running command cycles in place, a resting one is started (Solo's
    /// `restart-all`, distinct from `restart-running` which touches only the running ones).
    /// Untrusted commands are skipped. Reuses the per-process [`restart`](Supervisor::restart),
    /// so the trust re-check and crash-tracking reset are never reimplemented.
    pub fn restart_all_commands(&self, project: ProjectId) -> Result<(), SupervisorError> {
        self.restart_each(self.registry.commands_in(project))
    }

    /// Restarts each process in turn, tolerating an untrusted or vanished one (skipped) and
    /// surfacing only a durable-store failure — the shared loop behind the bulk restarts.
    fn restart_each(&self, ids: Vec<ProcessId>) -> Result<(), SupervisorError> {
        for id in ids {
            match self.restart(id) {
                // `restart` never yields `NotResumable` (only `resume` does); listed for
                // exhaustiveness alongside the other tolerated, non-fatal skips.
                Ok(())
                | Err(SupervisorError::Untrusted)
                | Err(SupervisorError::NotFound(_))
                | Err(SupervisorError::NotResumable(_)) => {}
                Err(err @ SupervisorError::Store(_)) => return Err(err),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "bulk_tests.rs"]
mod tests;
