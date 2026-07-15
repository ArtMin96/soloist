//! [`ScopedFacade`] — everything a session-scoped caller (MCP today, any future remote surface)
//! may do, and nothing else.
//!
//! Each method resolves the calling session's **effective-project scope** before routing to the
//! one core behaviour: an action can touch only a process within its project, and the trust gate
//! in C2 still refuses an untrusted command. Scope is resolved here, in the core, so every remote
//! adapter inherits the identical guarantee instead of re-checking it per adapter.
//!
//! Scope is a **type**, not a convention. [`Facade`] hands out whole contexts by reference
//! (`supervisor()`, `projects()`, …) because the local user driving the Tauri UI is not
//! scope-limited; a caller who *is* limited must not be able to reach those, and here it cannot:
//! `ScopedFacade` exposes no accessor, so there is no ungated door to pick by accident. Binding
//! the session once also takes it out of every signature — a caller cannot pass the wrong one.

use std::time::Duration;

use super::{Facade, LaunchAgentError};
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::ports::StoreError;
use crate::process::ProcessView;
use crate::supervisor::{StartSummary, SupervisorError};

/// The session-scoped view of the core: one caller's authority, bound to its session.
///
/// Borrows the [`Facade`] rather than sharing it, so an adapter already holding one (`&Facade`,
/// `Arc<Facade>`) makes a scoped view per request for free.
///
/// The guarantee is that this type has no way out. [`Facade`] hands whole contexts to the local
/// user (`supervisor()`, `projects()`, `trust()`, `agents()`, `config()`); a scope-limited caller
/// holding only a `ScopedFacade` cannot reach any of them, so it cannot route around its own
/// guard. That is enforced by the compiler, not by reviewers noticing:
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.supervisor();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.projects();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.trust();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.agents();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.config();
/// # }
/// ```
pub struct ScopedFacade<'a> {
    // Visible to the sibling modules that carry the rest of this type's methods, and no wider:
    // outside `facade` there is no way to reach the unscoped core through this view.
    pub(in crate::facade) inner: &'a Facade,
    pub(in crate::facade) session: SessionId,
}

impl Facade {
    /// This core as `session` may act on it: only that session's scoped surface, with no
    /// accessor onto an ungated context.
    pub fn scoped(&self, session: SessionId) -> ScopedFacade<'_> {
        ScopedFacade {
            inner: self,
            session,
        }
    }
}

impl ScopedFacade<'_> {
    /// The session this view acts as — for an adapter that must name it on the wire.
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// The session's effective project, or the coordination refusal when it has none.
    pub(in crate::facade) fn coordination_scope(
        &self,
    ) -> Result<ProjectId, super::CoordinationError> {
        self.inner.coordination_scope(self.session)
    }

    /// The process this session owns coordination state as, or the coordination refusal when it
    /// is not bound to one.
    pub(in crate::facade) fn coordination_owner(
        &self,
    ) -> Result<ProcessId, super::CoordinationError> {
        self.inner.coordination_owner(self.session)
    }
}

/// How many trailing rendered lines `send_input`'s `wait_ms` snapshot returns — a bounded
/// tail (about a screenful), never the whole scrollback, so the reply stays small.
const INPUT_TAIL_LINES: usize = 24;

/// The longest `send_input` waits before snapshotting the tail, regardless of the requested
/// `wait_ms`. A bound (per the longevity rules) so a large value cannot tie up the request,
/// and it stays well under the IPC client's request timeout.
const MAX_INPUT_WAIT: Duration = Duration::from_secs(10);

/// Why a session-scoped process action was refused. The wire adapters map this to their
/// own error type, so the taxonomy is defined once here.
#[derive(Debug, thiserror::Error)]
pub enum ScopedActionError {
    /// No process is registered under that id.
    #[error("no such process")]
    UnknownProcess,
    /// The session has no project in scope to act within (none selected, bound, or singular).
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The process exists but belongs to a different project than the session's scope.
    #[error("that process belongs to a different project")]
    OutOfScope,
    /// The command is not trusted to run in this project (the C2 trust gate refused it).
    #[error("command is not trusted to run in this project")]
    Untrusted,
    /// A durable read failed while resolving scope.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why spawning a worker agent over a scoped session failed: no project is in scope, the
/// caller is itself a spawned worker, or the underlying launch failed (unknown tool, unknown
/// project, store, or supervisor).
#[derive(Debug, thiserror::Error)]
pub enum SpawnAgentError {
    /// The session has no project in scope to spawn the worker into.
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The calling session is bound to a process that was itself spawned as a worker this
    /// run — delegation is one level deep, so a worker may not spawn its own workers.
    #[error("a worker agent cannot spawn agents; report back to the lead that spawned it")]
    WorkerMayNotSpawn,
    /// The launch itself failed — see [`LaunchAgentError`].
    #[error(transparent)]
    Launch(#[from] LaunchAgentError),
}

impl From<SupervisorError> for ScopedActionError {
    /// Projects a supervisor refusal onto the scoped taxonomy. The scope guard runs first, so
    /// a `NotFound` here means the process was forgotten between checks — reported as unknown.
    fn from(err: SupervisorError) -> Self {
        match err {
            SupervisorError::NotFound(_) => ScopedActionError::UnknownProcess,
            SupervisorError::Untrusted => ScopedActionError::Untrusted,
            SupervisorError::Store(err) => ScopedActionError::Store(err),
            // Resume is a local-only affordance (the UI's "Resume last session"), never a
            // scoped session action, so a scoped call cannot produce this — map it to the
            // closest scoped refusal for exhaustiveness.
            SupervisorError::NotResumable(_) => ScopedActionError::UnknownProcess,
        }
    }
}

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
    /// [`SpawnAgentError::WorkerMayNotSpawn`]. Must run within a `tokio` runtime (starting
    /// spawns the actor).
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
        // registered, or recorded.
        if let Some(caller) = self.inner.identity.origin(self.session).process() {
            if self.inner.lineage.parent_of(caller).is_some() {
                return Err(SpawnAgentError::WorkerMayNotSpawn);
            }
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

    /// Clears one in-scope process's output buffers (rendered and raw) without stopping it
    /// or touching its PTY. A scoped action — unlike the open output *reads*, clearing
    /// mutates what every viewer sees, so it is confined to the session's project. Returns
    /// whether the process had a terminal to clear.
    pub fn clear_output(&self, process: ProcessId) -> Result<bool, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self.inner.supervisor().clear_output(process))
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

    /// The recent rendered output of one in-scope process — the scoped `get_process_output`,
    /// bounded exactly as the open [`process_output`](Self::process_output) it delegates to.
    /// An out-of-scope process is refused, so an agent cannot read another project's logs
    /// (which can carry secrets).
    pub fn process_output_scoped(
        &self,
        process: ProcessId,
        lines: Option<usize>,
    ) -> Result<Vec<String>, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self
            .inner
            .process_output(process, lines)
            .unwrap_or_default())
    }

    /// The raw byte output of one in-scope process — the scoped `get_process_raw_output`.
    pub fn process_raw_output_scoped(
        &self,
        process: ProcessId,
    ) -> Result<Vec<u8>, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self.inner.process_raw_output(process).unwrap_or_default())
    }

    /// Rendered output lines of one in-scope process containing `query` — the scoped
    /// `search_output`.
    pub fn search_output_scoped(
        &self,
        process: ProcessId,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self
            .inner
            .search_output(process, query, limit)
            .unwrap_or_default())
    }

    /// Raw output lines of one in-scope process containing `query` — the scoped
    /// `search_raw_output`.
    pub fn search_raw_output_scoped(
        &self,
        process: ProcessId,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self
            .inner
            .search_raw_output(process, query, limit)
            .unwrap_or_default())
    }

    /// The listening ports of one in-scope process — the scoped `get_process_ports`.
    pub fn process_ports_scoped(&self, process: ProcessId) -> Result<Vec<u16>, ScopedActionError> {
        Ok(self.resolve_in_scope(process)?.ports)
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

    /// Resolves the session's effective project for a project-wide action, or
    /// `NoProjectScope` when none is selected, bound, or singular.
    fn scope(&self) -> Result<ProjectId, ScopedActionError> {
        self.inner
            .effective_project(self.session)
            .ok_or(ScopedActionError::NoProjectScope)
    }

    /// The scope guard, returning the in-scope process's view: the process must exist and
    /// belong to the session's effective project, else `UnknownProcess`/`OutOfScope`. The
    /// scoped actions and reads share this one resolution, so the rule lives in a single place.
    fn resolve_in_scope(&self, process: ProcessId) -> Result<ProcessView, ScopedActionError> {
        let view = self
            .inner
            .process_view(process)
            .ok_or(ScopedActionError::UnknownProcess)?;
        if view.project != self.scope()? {
            return Err(ScopedActionError::OutOfScope);
        }
        Ok(view)
    }

    /// The scope guard when the caller needs only the pass/fail, not the view. Public for the
    /// one remote read whose own return shape differs from the scoped reads — the async
    /// `wait_for_bound_port`, which confirms scope, then awaits — so its cross-project
    /// port-bind probe is refused like the other reads. The scope *rule* still lives here.
    pub fn require_in_scope(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.resolve_in_scope(process).map(|_| ())
    }
}

#[cfg(test)]
#[path = "scoped_tests.rs"]
mod tests;
