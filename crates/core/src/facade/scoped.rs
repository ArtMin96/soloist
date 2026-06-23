//! Session-scoped process actions (context C8) — the action surface a remote caller
//! (MCP today, the HTTP API later) drives.
//!
//! Each method enforces the calling session's **effective-project scope** before routing
//! to the one supervisor behaviour: a tool can act only on a process within its project
//! (parity F13), and the trust gate in C2 still refuses an untrusted command. The Tauri UI
//! calls the supervisor directly because the local user is not scope-limited; these methods
//! add scope on top for callers that are. Scope is resolved here, in the core, so every
//! remote adapter inherits the identical guarantee instead of re-checking it per adapter.

use super::Facade;
use crate::ids::{ProcessId, SessionId};
use crate::ports::StoreError;
use crate::supervisor::SupervisorError;

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
    /// The process exists but belongs to a different project than the session's scope (F13).
    #[error("that process belongs to a different project")]
    OutOfScope,
    /// The command is not trusted to run in this project (the C2 trust gate refused it).
    #[error("command is not trusted to run in this project")]
    Untrusted,
    /// A durable read failed while resolving scope.
    #[error(transparent)]
    Store(#[from] StoreError),
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
    /// Starts one process for a scoped session, after confirming it is in scope (F13). The
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

    /// The F13 guard: the process must exist and belong to the session's effective project.
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
        let scope = self
            .effective_project(session)
            .ok_or(ScopedActionError::NoProjectScope)?;
        if view.project != scope {
            return Err(ScopedActionError::OutOfScope);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "scoped_tests.rs"]
mod tests;
