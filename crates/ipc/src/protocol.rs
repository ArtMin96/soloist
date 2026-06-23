//! The request/reply protocol between an IPC client (the MCP server) and the app.
//!
//! Each [`IpcRequest`] maps to exactly one `Facade` behaviour; the server pairs it with
//! the connection's identity session for scope. Replies reuse the core read-model types
//! ([`ProcessView`]) so the wire shape can never drift from the domain — except a project
//! is sent as a lean [`ProjectSummary`] (no UI icon blob, which an agent does not need).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use soloist_core::{
    IdentityError, ProcessId, ProcessView, ProjectId, ProjectView, ScopedActionError, Whoami,
};

/// A request from an IPC client to the running app. The server resolves identity and
/// scope from the connection's session, so requests carry no session of their own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Who the caller is and which project its scoped tools act on.
    Whoami,
    /// Bind this session to the supervised process it runs in.
    BindSessionProcess { process: ProcessId },
    /// Register this session as an external caller under a label.
    RegisterAgent { label: String },
    /// Set this session's effective project scope.
    SelectProject { project: ProjectId },
    /// Every loaded project (not scope-filtered).
    ListProjects,
    /// One project with its processes; the effective scope when `project` is omitted.
    GetProjectStatus { project: Option<ProjectId> },
    /// Every managed process (not scope-filtered).
    ListProcesses,
    /// One process's current read-model row.
    GetProcessStatus { process: ProcessId },
    /// Start one process, scoped to the session's effective project (trust-gated).
    StartProcess { process: ProcessId },
    /// Gracefully stop one process, scoped to the session's effective project.
    StopProcess { process: ProcessId },
    /// Restart one process, scoped to the session's effective project (trust-gated).
    RestartProcess { process: ProcessId },
    /// Write input to one process's PTY (text or raw control bytes), scoped to the session.
    /// With `wait_ms`, the app waits then returns the rendered tail.
    SendInput {
        process: ProcessId,
        input: String,
        wait_ms: Option<u64>,
    },
}

/// A successful reply. The server always returns the variant matching the request.
///
/// Adjacently tagged (`{"ok": <variant>, "data": <payload>}`): the list variants wrap a
/// sequence, which serde cannot serialize under an *internal* tag (there is no map to inject
/// the tag into), so the payload goes in its own `data` field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ok", content = "data", rename_all = "snake_case")]
pub enum IpcResponse {
    /// The resolved identity (answer to [`IpcRequest::Whoami`]).
    Whoami(Whoami),
    /// A state-setting request succeeded (bind / register / select).
    Acked,
    /// Every loaded project.
    Projects(Vec<ProjectSummary>),
    /// One project with its processes.
    ProjectStatus(ProjectStatus),
    /// Every managed process.
    Processes(Vec<ProcessView>),
    /// One process's read-model row.
    Process(ProcessView),
    /// A stop request succeeded; the payload is whether the process was live when stopped.
    Stopped(bool),
    /// Input was written; the rendered tail when `wait_ms` was given, else `None`.
    InputSent(Option<String>),
}

/// The agent-facing projection of a project: its identity and root, without the UI's
/// icon data-URL. Built from the core [`ProjectView`] so the id stays single-source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
}

impl ProjectSummary {
    /// Projects a [`ProjectView`] to the lean agent-facing shape, dropping the icon.
    pub fn from_view(view: &ProjectView) -> Self {
        Self {
            id: view.id,
            name: view.name.clone(),
            root: view.root.clone(),
        }
    }
}

/// A project with its current processes — the answer to [`IpcRequest::GetProjectStatus`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectStatus {
    pub project: ProjectSummary,
    pub processes: Vec<ProcessView>,
}

/// Why a request failed: a typed error the client maps to a clear MCP tool error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum IpcError {
    /// The referenced process is not registered.
    #[error("no such process")]
    UnknownProcess,
    /// The referenced project is not loaded.
    #[error("no such project")]
    UnknownProject,
    /// A scoped request was made with no project in scope.
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The referenced process belongs to a different project than the session's scope.
    #[error("that process belongs to a different project")]
    OutOfScope,
    /// An action targeted a command that is not trusted to run in this project.
    #[error("command is not trusted to run in this project")]
    Untrusted,
    /// The app failed to serve the request (e.g. a durable read failed).
    #[error("the app could not serve the request: {0}")]
    Internal(String),
}

impl From<IdentityError> for IpcError {
    fn from(err: IdentityError) -> Self {
        match err {
            IdentityError::UnknownProcess => IpcError::UnknownProcess,
            IdentityError::UnknownProject => IpcError::UnknownProject,
            IdentityError::Store(err) => IpcError::Internal(err.to_string()),
        }
    }
}

impl From<ScopedActionError> for IpcError {
    fn from(err: ScopedActionError) -> Self {
        match err {
            ScopedActionError::UnknownProcess => IpcError::UnknownProcess,
            ScopedActionError::NoProjectScope => IpcError::NoProjectScope,
            ScopedActionError::OutOfScope => IpcError::OutOfScope,
            ScopedActionError::Untrusted => IpcError::Untrusted,
            ScopedActionError::Store(err) => IpcError::Internal(err.to_string()),
        }
    }
}

/// A framed reply: success or a typed failure.
pub type IpcResult = Result<IpcResponse, IpcError>;

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
