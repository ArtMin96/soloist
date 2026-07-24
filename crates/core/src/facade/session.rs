//! Identity sessions and effective-project scope (context C8) — the façade surface a remote
//! caller (MCP today, the HTTP API later) drives to say who it is and which project its tools
//! act on.
//!
//! A session opens with the connecting peer's process group (the transport adapter reads it
//! from the kernel). A bind or a project selection is authenticated against that group: a
//! session can only bind to the process it runs in and only select a project it runs in, so
//! the effective-project scope the [scoped actions](super::scoped) trust is unforgeable. The
//! Tauri UI never opens a session — the local user is not scope-limited.

use super::scoped::ScopedFacade;
use super::Facade;
use crate::identity::{IdentityError, PeerCredentials, Whoami};
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::projects::ProjectRef;

impl Facade {
    /// Opens an identity session for a new MCP connection (C8). The IPC server holds the
    /// returned [`SessionId`] for the life of the connection and passes it on every call,
    /// so each tool acts under the right identity and project scope. `peer` is the connecting
    /// peer's [`PeerCredentials`], read from the kernel by the transport adapter (both facts
    /// `None` when it cannot authenticate the peer); a bind or project selection is matched
    /// against them, so a session can only scope to a process — or a project directory — it
    /// actually runs in.
    pub fn open_session(&self, peer: PeerCredentials) -> SessionId {
        self.identity.open(peer)
    }

    /// The lean id-and-name reference for a resolved effective project. The id is authoritative —
    /// it comes from in-memory identity — while the name is a best-effort durable-store read that
    /// resolves to `None` when the store cannot be read or the record is gone. So a transient store
    /// error dims the name without ever dropping the scope the caller still holds.
    fn project_ref(&self, id: ProjectId) -> ProjectRef {
        match self.projects.get(id).ok().flatten() {
            Some(record) => ProjectRef::from_record(&record),
            None => ProjectRef { id, name: None },
        }
    }

    /// The project a session's scoped tools act on: its explicit selection, else the project
    /// owning its bound process, else the project its connecting peer's working directory sits
    /// inside, else the sole loaded project when there is exactly one — otherwise `None`
    /// (ambiguous; a scoped tool must ask the caller to `select_project`). Best-effort: a store
    /// read error resolves to `None` rather than failing `whoami`. The selection, bound process,
    /// and working directory are each authenticated (the selection and bind at their own time, the
    /// directory being a kernel-read fact), so every non-`None` resolution is a project the caller
    /// runs in (the sole-project default is the one unambiguous exception).
    pub fn effective_project(&self, session: SessionId) -> Option<ProjectId> {
        if let Some(project) = self.identity.selected_project(session) {
            return Some(project);
        }
        if let Some(process) = self.identity.origin(session).process() {
            if let Some(view) = self.process_view(process) {
                return Some(view.project);
            }
        }
        if let Some(project) = self.project_at_peer_cwd(session) {
            return Some(project);
        }
        match self.projects.list() {
            Ok(projects) if projects.len() == 1 => projects.first().map(|record| record.id),
            _ => None,
        }
    }

    /// The loaded project the session's connecting peer runs *in*, resolved from the kernel-read
    /// working directory the transport authenticated — the open project whose root contains that
    /// directory (deepest wins). This is the authenticated scope signal for an agent Soloist did
    /// not launch (no managed process in its group); it is the counterpart to the process-group
    /// signal for one it did, and both the scope resolution and the `select_project` authenticity
    /// check share it. `None` when the peer supplied no directory or no open project contains it;
    /// best-effort, so a store read error also resolves to `None`.
    fn project_at_peer_cwd(&self, session: SessionId) -> Option<ProjectId> {
        let cwd = self.identity.peer_cwd(session)?;
        self.projects.project_at_path(&cwd).ok().flatten()
    }
}

impl ScopedFacade<'_> {
    /// Closes an identity session when its connection ends, dropping its state.
    pub fn close_session(&self) {
        self.inner.identity.close(self.session);
    }

    /// Binds a session to the supervised process it runs in — the process whose
    /// [`PROCESS_ID_ENV`](crate::ids::PROCESS_ID_ENV) the agent's MCP client read.
    /// Fails [`UnknownProcess`](IdentityError::UnknownProcess) if no such process is
    /// registered, or [`ForeignProcess`](IdentityError::ForeignProcess) if the binding is not
    /// authentic — the caller's connecting peer does not run in that process's group. The
    /// authenticity check is what makes the effective-project scope trustworthy: a client on
    /// the local socket cannot bind to a sibling project's process it does not run in.
    pub fn bind_session_process(&self, process: ProcessId) -> Result<(), IdentityError> {
        if self.inner.supervisor.label_of(process).is_none() {
            return Err(IdentityError::UnknownProcess);
        }
        if self.home_process() != Some(process) {
            return Err(IdentityError::ForeignProcess);
        }
        self.inner.identity.bind_process(self.session, process);
        Ok(())
    }

    /// Registers an external caller (one with no Soloist-supervised process) under a
    /// label, so `whoami` can report who it is.
    pub fn register_agent(&self, label: String) {
        self.inner.identity.register_external(self.session, label);
    }

    /// Sets a session's effective project scope explicitly. Fails
    /// [`UnknownProject`](IdentityError::UnknownProject) if the project is not loaded, or
    /// [`ForeignProject`](IdentityError::ForeignProject) if the caller does not run in it —
    /// no process in the caller's own process group belongs to it. A session can therefore
    /// only select a project it actually runs in, never a sibling on the shared local socket.
    pub fn select_project(&self, project: ProjectId) -> Result<(), IdentityError> {
        if self.inner.projects.get(project)?.is_none() {
            return Err(IdentityError::UnknownProject);
        }
        if self.home_project() != Some(project) {
            return Err(IdentityError::ForeignProject);
        }
        self.inner.identity.select_project(self.session, project);
        Ok(())
    }

    /// Records a session's selected process — an informational default-target hint reported by
    /// `whoami`. Unlike [`select_project`](Self::select_project) it confers no scope or
    /// authority: every scoped tool takes an explicit process id, so the selection is a
    /// convenience marker, not a gate, and needs no peer-group authentication. It is still
    /// confined to the caller's effective project, though: a process outside scope is reported
    /// as [`UnknownProcess`](IdentityError::UnknownProcess), indistinguishable from one that
    /// does not exist, so selecting can never confirm the existence of another project's
    /// processes. Fails `UnknownProcess` when the process is not in the session's scope.
    pub fn select_process(&self, process: ProcessId) -> Result<(), IdentityError> {
        let in_scope = self
            .inner
            .effective_project(self.session)
            .zip(self.inner.process_view(process))
            .is_some_and(|(scope, view)| view.project == scope);
        if !in_scope {
            return Err(IdentityError::UnknownProcess);
        }
        self.inner.identity.select_process(self.session, process);
        Ok(())
    }

    /// Resolves who a session is and the project its scoped tools act on (the answer to
    /// the `whoami` tool), enriched with the bound process's details and the project's name.
    pub fn whoami(&self) -> Whoami {
        let origin = self.inner.identity.origin(self.session);
        Whoami {
            session: self.session,
            bound_process: origin
                .process()
                .and_then(|process| self.inner.process_view(process)),
            // Resolving the view also drops a selection whose process has since left the registry
            // (e.g. it was closed), so `whoami` never echoes a dangling id.
            selected_process: self
                .inner
                .identity
                .selected_process(self.session)
                .and_then(|process| self.inner.process_view(process)),
            effective_project: self
                .inner
                .effective_project(self.session)
                .map(|id| self.inner.project_ref(id)),
            origin,
        }
    }

    /// The managed process the session's connecting peer runs in (its *home* process),
    /// resolved from the kernel-reported peer process group via the supervisor — the
    /// unforgeable basis for authenticating a bind or a project selection, and for any gate
    /// that must hold whether or not the caller chose to bind. `None` when the transport
    /// supplied no peer group, or no live managed process owns it (an external caller, or a
    /// stale group).
    pub(in crate::facade) fn home_process(&self) -> Option<ProcessId> {
        self.inner
            .identity
            .peer_pgid(self.session)
            .and_then(|pgid| self.inner.supervisor.process_at_pgid(pgid))
    }

    /// The project the session's caller authentically runs in — the only project it can select or
    /// transfer to. It is the project of its home process (the group signal, for a Soloist-launched
    /// agent), else the project its connecting peer's working directory sits inside (the directory
    /// signal, for an agent Soloist did not launch). `None` when neither resolves.
    fn home_project(&self) -> Option<ProjectId> {
        self.home_process()
            .and_then(|id| self.inner.process_view(id))
            .map(|view| view.project)
            .or_else(|| self.inner.project_at_peer_cwd(self.session))
    }

    /// Whether `session` is authentically scoped to `project` — its connecting peer runs in it,
    /// the same check [`select_project`](Self::select_project) enforces. The cross-project
    /// transfer surface requires this of the **target** project (the source is the caller's own
    /// effective scope), so content only ever moves between two projects the caller actually runs
    /// in. Because a single connection authenticates to one project, a genuine cross-project
    /// transfer over MCP is refused; the reachable path is the local/trusted surface.
    pub(in crate::facade) fn authentic_scope(&self, project: ProjectId) -> bool {
        self.home_project() == Some(project)
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
