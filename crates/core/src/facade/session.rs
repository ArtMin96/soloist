//! Identity sessions and effective-project scope (context C8) — the façade surface a remote
//! caller (MCP today, the HTTP API later) drives to say who it is and which project its tools
//! act on.
//!
//! A session opens with the connecting peer's process group (the transport adapter reads it
//! from the kernel). A bind or a project selection is authenticated against that group: a
//! session can only bind to the process it runs in and only select a project it runs in, so
//! the effective-project scope the [scoped actions](super::scoped) trust is unforgeable. The
//! Tauri UI never opens a session — the local user is not scope-limited.

use super::Facade;
use crate::identity::{IdentityError, Whoami};
use crate::ids::{ProcessId, ProjectId, SessionId};

impl Facade {
    /// Opens an identity session for a new MCP connection (C8). The IPC server holds the
    /// returned [`SessionId`] for the life of the connection and passes it on every call,
    /// so each tool acts under the right identity and project scope. `peer_pgid` is the
    /// connecting peer's process group, read from the kernel by the transport adapter
    /// (`None` when it cannot authenticate the peer); a bind or project selection is matched
    /// against it, so a session can only scope to a process it actually runs in.
    pub fn open_session(&self, peer_pgid: Option<i32>) -> SessionId {
        self.identity.open(peer_pgid)
    }

    /// Closes an identity session when its connection ends, dropping its state.
    pub fn close_session(&self, session: SessionId) {
        self.identity.close(session);
    }

    /// Binds a session to the supervised process it runs in — the process whose
    /// [`PROCESS_ID_ENV`](crate::identity::PROCESS_ID_ENV) the agent's MCP client read.
    /// Fails [`UnknownProcess`](IdentityError::UnknownProcess) if no such process is
    /// registered, or [`ForeignProcess`](IdentityError::ForeignProcess) if the binding is not
    /// authentic — the caller's connecting peer does not run in that process's group. The
    /// authenticity check is what makes the effective-project scope trustworthy: a client on
    /// the local socket cannot bind to a sibling project's process it does not run in.
    pub fn bind_session_process(
        &self,
        session: SessionId,
        process: ProcessId,
    ) -> Result<(), IdentityError> {
        if self.supervisor.label_of(process).is_none() {
            return Err(IdentityError::UnknownProcess);
        }
        if self.home_process(session) != Some(process) {
            return Err(IdentityError::ForeignProcess);
        }
        self.identity.bind_process(session, process);
        Ok(())
    }

    /// Registers an external caller (one with no Soloist-supervised process) under a
    /// label, so `whoami` can report who it is.
    pub fn register_agent(&self, session: SessionId, label: String) {
        self.identity.register_external(session, label);
    }

    /// Sets a session's effective project scope explicitly. Fails
    /// [`UnknownProject`](IdentityError::UnknownProject) if the project is not loaded, or
    /// [`ForeignProject`](IdentityError::ForeignProject) if the caller does not run in it —
    /// no process in the caller's own process group belongs to it. A session can therefore
    /// only select a project it actually runs in, never a sibling on the shared local socket.
    pub fn select_project(
        &self,
        session: SessionId,
        project: ProjectId,
    ) -> Result<(), IdentityError> {
        if self.projects.get(project)?.is_none() {
            return Err(IdentityError::UnknownProject);
        }
        if self.home_project(session) != Some(project) {
            return Err(IdentityError::ForeignProject);
        }
        self.identity.select_project(session, project);
        Ok(())
    }

    /// Resolves who a session is and the project its scoped tools act on (the answer to
    /// the `whoami` tool).
    pub fn whoami(&self, session: SessionId) -> Whoami {
        let origin = self.identity.origin(session);
        Whoami {
            session,
            bound_process: origin.process(),
            effective_project: self.effective_project(session),
            origin,
        }
    }

    /// The project a session's scoped tools act on: its explicit selection, else the
    /// project owning its bound process, else the sole loaded project when there is
    /// exactly one — otherwise `None` (ambiguous; a scoped tool must ask the caller to
    /// `select_project`). Best-effort: a store read error resolves to `None` rather than
    /// failing `whoami`. The selection and bound process are themselves authenticated at
    /// bind/select time, so each non-`None` resolution is a project the caller runs in (the
    /// sole-project default is the one unambiguous exception).
    pub(crate) fn effective_project(&self, session: SessionId) -> Option<ProjectId> {
        if let Some(project) = self.identity.selected_project(session) {
            return Some(project);
        }
        if let Some(process) = self.identity.origin(session).process() {
            if let Some(view) = self.process_view(process) {
                return Some(view.project);
            }
        }
        match self.projects.list() {
            Ok(projects) if projects.len() == 1 => projects.first().map(|record| record.id),
            _ => None,
        }
    }

    /// The managed process the session's connecting peer runs in (its *home* process),
    /// resolved from the kernel-reported peer process group via the supervisor — the
    /// unforgeable basis for authenticating a bind or a project selection. `None` when the
    /// transport supplied no peer group, or no live managed process owns it (an external
    /// caller, or a stale group).
    fn home_process(&self, session: SessionId) -> Option<ProcessId> {
        self.identity
            .peer_pgid(session)
            .and_then(|pgid| self.supervisor.process_at_pgid(pgid))
    }

    /// The project the session's home process belongs to — the only project a caller can
    /// authentically select. `None` when the caller has no home process.
    fn home_project(&self, session: SessionId) -> Option<ProjectId> {
        self.home_process(session)
            .and_then(|id| self.process_view(id))
            .map(|view| view.project)
    }
}
