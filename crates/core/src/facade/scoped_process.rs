//! What a session-scoped caller may do to a process (context C8) — the scoped half of process
//! supervision.
//!
//! Each action resolves the session's effective-project scope before routing to the one C2
//! behaviour, so a scoped caller can act only within its own project and the trust gate still
//! refuses an untrusted command. The scope rule itself lives once in
//! [`scoped`](super::scoped); this module only spends it.

use std::time::Duration;

use super::scoped::{ScopedActionError, ScopedFacade, SpawnAgentError};
use crate::ids::{ProcessId, ProjectId};
use crate::process::ProcessView;
use crate::supervisor::StartSummary;

/// How many trailing rendered lines `send_input`'s `wait_ms` snapshot returns — a bounded
/// tail (about a screenful), never the whole scrollback, so the reply stays small.
const INPUT_TAIL_LINES: usize = 24;

/// The longest `send_input` waits before snapshotting the tail, regardless of the requested
/// `wait_ms`. A bound (per the longevity rules) so a large value cannot tie up the request,
/// and it stays well under the IPC client's request timeout.
pub(in crate::facade) const MAX_INPUT_WAIT: Duration = Duration::from_secs(10);

impl ScopedFacade<'_> {
    /// Starts one process for a scoped session, after confirming it is in scope. The
    /// trust gate in the supervisor still applies, so an untrusted command is refused.
    pub fn start_process(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().start(process)?;
        Ok(())
    }

    /// Requests a graceful stop of one in-scope process, returning whether it was live.
    pub fn stop_process(&self, process: ProcessId) -> Result<bool, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self.inner.supervisor().stop(process))
    }

    /// Restarts one in-scope process (stop then start with its saved config); trust-gated.
    pub fn restart_process(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().restart(process)?;
        Ok(())
    }

    /// Renames one in-scope process's display label. A scoped action — the label is shared
    /// read-model state every viewer sees — so it is confined to the session's project.
    /// Ungated by trust: a rename runs nothing.
    pub fn rename_process(
        &self,
        process: ProcessId,
        label: String,
    ) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().rename(process, label)?;
        Ok(())
    }

    /// Closes one in-scope process: stops and reaps it, then removes it from the registry. A
    /// scoped action confined to the session's project. Async because it awaits the group's
    /// reap before the process is forgotten, so no child is abandoned. Ungated by trust:
    /// stopping and forgetting a process runs nothing.
    pub async fn close_process(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().close(process).await?;
        Ok(())
    }

    /// Writes input to an in-scope process's PTY — UTF-8 text, including control characters,
    /// sent verbatim (include `\r` to submit a line, `\u{3}` for Ctrl-C). When `wait` is set, waits
    /// up to [`MAX_INPUT_WAIT`] for the process to react, then returns the rendered terminal
    /// tail so the caller sees the effect; without it, returns `None` immediately. The clock
    /// is injected, so a test drives the wait without real time passing.
    pub async fn send_input(
        &self,
        process: ProcessId,
        input: Vec<u8>,
        wait: Option<Duration>,
    ) -> Result<Option<String>, ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().write_stdin(process, input).await?;
        let Some(wait) = wait else {
            return Ok(None);
        };
        self.inner.clock.sleep(wait.min(MAX_INPUT_WAIT)).await;
        Ok(self
            .inner
            .supervisor()
            .rendered_tail(process, INPUT_TAIL_LINES)
            .map(|lines| lines.join("\n")))
    }

    /// Spawns a configured agent tool as a worker in the session's effective project and
    /// starts it, returning its process id — a lead agent spawning a worker over MCP. Reuses
    /// [`Facade::launch_agent`] for the one launch behaviour; the worker always
    /// lands in the caller's own project (the resolved scope), so it can never spawn into
    /// another and needs no project argument. The new agent auto-binds via the injected
    /// `SOLOIST_PROCESS_ID`. When the calling session is bound to a lead process, the worker's
    /// lineage is recorded under that lead so the orchestration tree nests it; an unbound or
    /// external caller's spawn is a root. Delegation is one level deep: a caller that was
    /// itself spawned as a worker this run is refused with
    /// [`SpawnAgentError::WorkerMayNotSpawn`], whether it identified itself by binding or is
    /// recognised by the process group it connects from. Must run within a `tokio` runtime
    /// (starting spawns the actor).
    pub fn spawn_agent(
        &self,
        tool: &str,
        extra_args: Vec<String>,
    ) -> Result<ProcessId, SpawnAgentError> {
        let project = self
            .inner
            .effective_project(self.session)
            .ok_or(SpawnAgentError::NoProjectScope)?;
        // Delegation is one level deep: a caller recorded as a spawned worker is refused for
        // its whole run — deliberately unfiltered by parent liveness, so a closed lead never
        // promotes its workers to spawners. Refusal precedes the launch: nothing is spawned,
        // registered, or recorded. The caller is resolved from the kernel-reported peer group
        // as well as from its own binding, so a worker cannot lift the gate by never binding.
        let caller_is_worker = [
            self.home_process(),
            self.inner.identity.origin(self.session).process(),
        ]
        .into_iter()
        .flatten()
        .any(|caller| self.inner.lineage.parent_of(caller).is_some());
        if caller_is_worker {
            return Err(SpawnAgentError::WorkerMayNotSpawn);
        }
        let worker = self.inner.launch_agent(project, tool, extra_args)?;
        // A worker spawned by a bound lead nests under it in the orchestration tree; an
        // unbound or external caller's spawn records no parent and so reads back as a root.
        if let Some(lead) = self.inner.identity.origin(self.session).process() {
            self.inner.lineage.record(worker, lead);
        }
        Ok(worker)
    }

    /// Starts every trusted command in the session's effective project, regardless of
    /// `auto_start` — the scoped `start_all_commands` tool. Returns what started and what was
    /// skipped as untrusted. Distinct from the dashboard's auto-start path; an untrusted
    /// command is reported, never run.
    pub fn start_all_commands(&self) -> Result<StartSummary, ScopedActionError> {
        let project = self.scope()?;
        Ok(self.inner.supervisor().start_all_commands(project)?)
    }

    /// Gracefully stops every running command in the session's effective project (leaving
    /// agents and terminals running), returning how many were messaged.
    pub fn stop_all_commands(&self) -> Result<usize, ScopedActionError> {
        let project = self.scope()?;
        Ok(self.inner.supervisor().stop_all_commands(project))
    }

    /// Restarts every trusted command in the session's effective project — running ones
    /// cycle, resting ones start — bringing the command set up fresh. Untrusted skipped.
    pub fn restart_all_commands(&self) -> Result<(), ScopedActionError> {
        let project = self.scope()?;
        self.inner.supervisor().restart_all_commands(project)?;
        Ok(())
    }

    /// Every process, with rows outside the session's effective project reduced to identity
    /// — the scoped `list_processes`. A caller keeps a cross-project overview (which projects
    /// and processes exist) but reads no foreign project's ports, exit code, or output-derived
    /// state. With no project in scope every row is foreign, so all are redacted.
    pub fn snapshot_scoped(&self) -> Vec<ProcessView> {
        let scope = self.inner.effective_project(self.session);
        self.inner
            .snapshot()
            .into_iter()
            .map(|view| {
                if Some(view.project) == scope {
                    view
                } else {
                    view.redacted_identity()
                }
            })
            .collect()
    }

    /// The processes in `project`, with rows outside the session's effective project reduced to
    /// identity — the same rule [`snapshot_scoped`](Self::snapshot_scoped) applies, narrowed to one
    /// project.
    ///
    /// Naming a project is not itself a disclosure (`list_projects` already lists them all), so a
    /// foreign project reads back as the bare identities of its processes and nothing more: no
    /// ports, exit code, or output-derived state. Composing this from an unscoped snapshot in an
    /// adapter is what let a scoped caller read a foreign project's rows in full.
    pub fn project_processes_scoped(&self, project: ProjectId) -> Vec<ProcessView> {
        let scope = self.inner.effective_project(self.session);
        self.inner
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project)
            .map(|view| {
                if Some(view.project) == scope {
                    view
                } else {
                    view.redacted_identity()
                }
            })
            .collect()
    }

    /// The services of the session's effective project: its command processes with their
    /// status, discovered ports, and readiness (the [`ProcessView`] read model). Scoped to
    /// the project so a caller sees only its own services; agents and terminals are omitted.
    pub fn services_list(&self) -> Result<Vec<ProcessView>, ScopedActionError> {
        let project = self.scope()?;
        Ok(self
            .inner
            .snapshot()
            .into_iter()
            .filter(|view| view.is_command_in(project))
            .collect())
    }

    /// The status view of one in-scope process — the scoped `get_process_status`. Refuses a
    /// process outside the session's project rather than disclose its state across the
    /// project-isolation boundary; the open [`process_view`](Self::process_view) stays for
    /// the local (unscoped) UI and the HTTP API.
    pub fn process_status_scoped(
        &self,
        process: ProcessId,
    ) -> Result<ProcessView, ScopedActionError> {
        self.resolve_in_scope(process)
    }
}
