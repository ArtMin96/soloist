//! Caller identity and effective-project scope (context C8).
//!
//! Each connection from an MCP client is one session. A process Soloist launched finds
//! its own [`ProcessId`] in the [`PROCESS_ID_ENV`] variable Soloist injects and binds
//! its session to it; an external client registers under a label instead. The bound
//! process, or an explicit [`select_project`](Identity::select_project) choice,
//! determines the effective project a scoped tool acts on — composed by the façade,
//! which alone can see the project registry and the supervisor.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::ports::StoreError;
use crate::sync::lock;

/// The environment variable Soloist injects into every managed process, carrying that
/// process's own [`ProcessId`] as a decimal string. An agent that launches the MCP
/// server reads it and calls `bind_session_process`, so its tool calls are attributed
/// to — and scoped by — the process it runs in.
pub const PROCESS_ID_ENV: &str = "SOLOIST_PROCESS_ID";

/// Who a session's caller is. A session starts [`Unbound`](Origin::Unbound); a
/// Soloist-supervised process binds to itself, while an external client registers a
/// label.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Origin {
    /// The caller has not identified itself.
    #[default]
    Unbound,
    /// A Soloist-supervised process, bound via [`PROCESS_ID_ENV`].
    Process(ProcessId),
    /// An external client that registered under a label.
    External(String),
}

impl Origin {
    /// The process this origin is bound to, if it is a supervised process.
    pub fn process(&self) -> Option<ProcessId> {
        match self {
            Origin::Process(id) => Some(*id),
            Origin::Unbound | Origin::External(_) => None,
        }
    }
}

/// The mutable state of one session: who the caller is and the project it explicitly
/// selected (if any).
#[derive(Clone, Debug, Default)]
struct Session {
    origin: Origin,
    selected_project: Option<ProjectId>,
}

/// What a session resolves to: its caller [`Origin`], the process it is bound to (if
/// any), and the effective project a scoped tool would act on (if one can be resolved).
/// The answer the `whoami` tool returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Whoami {
    pub session: SessionId,
    pub origin: Origin,
    pub bound_process: Option<ProcessId>,
    pub effective_project: Option<ProjectId>,
}

/// Why an identity command failed: the referenced process or project is not registered,
/// or the durable store could not be read while validating a selection.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// `bind_session_process` named a process that is not in the registry.
    #[error("no such process")]
    UnknownProcess,
    /// `select_project` named a project that is not loaded.
    #[error("no such project")]
    UnknownProject,
    /// The project store could not be read.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The per-session identity registry (C8): the source of truth for which process each
/// session is bound to and which project it selected. Pure in-memory state with
/// interior mutability — the façade composes the *effective* project from this plus the
/// project registry and the supervisor.
#[derive(Default)]
pub struct Identity {
    sessions: Mutex<HashMap<SessionId, Session>>,
}

impl Identity {
    /// A registry with no open sessions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a fresh session for a new MCP connection and returns its id.
    pub fn open(&self) -> SessionId {
        let id = SessionId::next();
        lock(&self.sessions).insert(id, Session::default());
        id
    }

    /// Binds a session to the supervised process it runs in (from [`PROCESS_ID_ENV`]).
    pub fn bind_process(&self, session: SessionId, process: ProcessId) {
        self.update(session, |s| s.origin = Origin::Process(process));
    }

    /// Registers an external (non-supervised) caller under a label.
    pub fn register_external(&self, session: SessionId, label: String) {
        self.update(session, |s| s.origin = Origin::External(label));
    }

    /// Sets the session's explicitly selected project scope.
    pub fn select_project(&self, session: SessionId, project: ProjectId) {
        self.update(session, |s| s.selected_project = Some(project));
    }

    /// Drops a session's state when its connection ends.
    pub fn close(&self, session: SessionId) {
        lock(&self.sessions).remove(&session);
    }

    /// The caller origin recorded for a session ([`Origin::Unbound`] if unknown).
    pub fn origin(&self, session: SessionId) -> Origin {
        lock(&self.sessions)
            .get(&session)
            .map(|s| s.origin.clone())
            .unwrap_or_default()
    }

    /// The project a session explicitly selected, if any.
    pub fn selected_project(&self, session: SessionId) -> Option<ProjectId> {
        lock(&self.sessions)
            .get(&session)
            .and_then(|s| s.selected_project)
    }

    /// Applies `f` to a session's state, creating the entry if the session was never
    /// opened — so a bind or select is idempotent even on a not-yet-opened session.
    fn update(&self, session: SessionId, f: impl FnOnce(&mut Session)) {
        f(lock(&self.sessions).entry(session).or_default());
    }
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
