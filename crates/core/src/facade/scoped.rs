//! Session-scoped process actions and queries (context C8) — the surface a remote caller
//! (MCP today, the HTTP API later) drives within its effective project.
//!
//! Each method resolves the calling session's **effective-project scope** before routing
//! to the one supervisor behaviour: an action can touch only a process within its project,
//! and the trust gate in C2 still refuses an untrusted command. The Tauri UI
//! calls the supervisor directly because the local user is not scope-limited; these methods
//! add scope on top for callers that are. Scope is resolved here, in the core, so every
//! remote adapter inherits the identical guarantee instead of re-checking it per adapter.

use std::time::Duration;

use super::{Facade, LaunchAgentError};
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::ports::StoreError;
use crate::process::ProcessView;
use crate::supervisor::{StartSummary, SupervisorError};

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

/// Why spawning a worker agent over a scoped session failed: no project is in scope, or the
/// underlying launch failed (unknown tool, unknown project, store, or supervisor).
#[derive(Debug, thiserror::Error)]
pub enum SpawnAgentError {
    /// The session has no project in scope to spawn the worker into.
    #[error("no project is in scope; select one first")]
    NoProjectScope,
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
        }
    }
}

impl Facade {
    /// Starts one process for a scoped session, after confirming it is in scope. The
    /// trust gate in the supervisor still applies, so an untrusted command is refused.
    pub fn start_process(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<(), ScopedActionError> {
        self.require_in_scope(session, process)?;
        self.supervisor().start(process)?;
        Ok(())
    }

    /// Requests a graceful stop of one in-scope process, returning whether it was live.
    pub fn stop_process(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<bool, ScopedActionError> {
        self.require_in_scope(session, process)?;
        Ok(self.supervisor().stop(process))
    }

    /// Restarts one in-scope process (stop then start with its saved config); trust-gated.
    pub fn restart_process(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<(), ScopedActionError> {
        self.require_in_scope(session, process)?;
        self.supervisor().restart(process)?;
        Ok(())
    }

    /// Renames one in-scope process's display label. A scoped action — the label is shared
    /// read-model state every viewer sees — so it is confined to the session's project.
    /// Ungated by trust: a rename runs nothing.
    pub fn rename_process(
        &self,
        session: SessionId,
        process: ProcessId,
        label: String,
    ) -> Result<(), ScopedActionError> {
        self.require_in_scope(session, process)?;
        self.supervisor().rename(process, label)?;
        Ok(())
    }

    /// Closes one in-scope process: stops and reaps it, then removes it from the registry. A
    /// scoped action confined to the session's project. Async because it awaits the group's
    /// reap before the process is forgotten, so no child is abandoned. Ungated by trust:
    /// stopping and forgetting a process runs nothing.
    pub async fn close_process(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<(), ScopedActionError> {
        self.require_in_scope(session, process)?;
        self.supervisor().close(process).await?;
        Ok(())
    }

    /// Writes input to an in-scope process's PTY — UTF-8 text, including control characters,
    /// sent verbatim (include `\r` to submit a line, `\u{3}` for Ctrl-C). When `wait` is set, waits
    /// up to [`MAX_INPUT_WAIT`] for the process to react, then returns the rendered terminal
    /// tail so the caller sees the effect; without it, returns `None` immediately. The clock
    /// is injected, so a test drives the wait without real time passing.
    pub async fn send_input(
        &self,
        session: SessionId,
        process: ProcessId,
        input: Vec<u8>,
        wait: Option<Duration>,
    ) -> Result<Option<String>, ScopedActionError> {
        self.require_in_scope(session, process)?;
        self.supervisor().write_stdin(process, input).await?;
        let Some(wait) = wait else {
            return Ok(None);
        };
        self.clock.sleep(wait.min(MAX_INPUT_WAIT)).await;
        Ok(self
            .supervisor()
            .rendered_tail(process, INPUT_TAIL_LINES)
            .map(|lines| lines.join("\n")))
    }

    /// Spawns a configured agent tool as a worker in the session's effective project and
    /// starts it, returning its process id — a lead agent spawning a worker over MCP. Reuses
    /// [`Facade::launch_agent`] for the one launch behaviour; the worker always
    /// lands in the caller's own project (the resolved scope), so it can never spawn into
    /// another and needs no project argument. The new agent auto-binds via the injected
    /// `SOLOIST_PROCESS_ID`. Must run within a `tokio` runtime (starting spawns the actor).
    pub fn spawn_agent(
        &self,
        session: SessionId,
        tool: &str,
        extra_args: Vec<String>,
    ) -> Result<ProcessId, SpawnAgentError> {
        let project = self
            .effective_project(session)
            .ok_or(SpawnAgentError::NoProjectScope)?;
        Ok(self.launch_agent(project, tool, extra_args)?)
    }

    /// Starts every trusted command in the session's effective project, regardless of
    /// `auto_start` — the scoped `start_all_commands` tool. Returns what started and what was
    /// skipped as untrusted. Distinct from the dashboard's auto-start path; an untrusted
    /// command is reported, never run.
    pub fn start_all_commands(
        &self,
        session: SessionId,
    ) -> Result<StartSummary, ScopedActionError> {
        let project = self.scope(session)?;
        Ok(self.supervisor().start_all_commands(project)?)
    }

    /// Gracefully stops every running command in the session's effective project (leaving
    /// agents and terminals running), returning how many were messaged.
    pub fn stop_all_commands(&self, session: SessionId) -> Result<usize, ScopedActionError> {
        let project = self.scope(session)?;
        Ok(self.supervisor().stop_all_commands(project))
    }

    /// Restarts every trusted command in the session's effective project — running ones
    /// cycle, resting ones start — bringing the command set up fresh. Untrusted skipped.
    pub fn restart_all_commands(&self, session: SessionId) -> Result<(), ScopedActionError> {
        let project = self.scope(session)?;
        self.supervisor().restart_all_commands(project)?;
        Ok(())
    }

    /// Clears one in-scope process's output buffers (rendered and raw) without stopping it
    /// or touching its PTY. A scoped action — unlike the open output *reads*, clearing
    /// mutates what every viewer sees, so it is confined to the session's project. Returns
    /// whether the process had a terminal to clear.
    pub fn clear_output(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<bool, ScopedActionError> {
        self.require_in_scope(session, process)?;
        Ok(self.supervisor().clear_output(process))
    }

    /// The services of the session's effective project: its command processes with their
    /// status, discovered ports, and readiness (the [`ProcessView`] read model). Scoped to
    /// the project so a caller sees only its own services; agents and terminals are omitted.
    pub fn services_list(&self, session: SessionId) -> Result<Vec<ProcessView>, ScopedActionError> {
        let project = self.scope(session)?;
        Ok(self
            .snapshot()
            .into_iter()
            .filter(|view| view.is_command_in(project))
            .collect())
    }

    /// Resolves the session's effective project for a project-wide action, or
    /// `NoProjectScope` when none is selected, bound, or singular.
    fn scope(&self, session: SessionId) -> Result<ProjectId, ScopedActionError> {
        self.effective_project(session)
            .ok_or(ScopedActionError::NoProjectScope)
    }

    /// The scope guard: the process must exist and belong to the session's effective project.
    /// Returns `OutOfScope` rather than hiding a cross-project process, since the read tools
    /// already expose every process unfiltered (open by design).
    fn require_in_scope(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<(), ScopedActionError> {
        let view = self
            .process_view(process)
            .ok_or(ScopedActionError::UnknownProcess)?;
        if view.project != self.scope(session)? {
            return Err(ScopedActionError::OutOfScope);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "scoped_tests.rs"]
mod tests;
