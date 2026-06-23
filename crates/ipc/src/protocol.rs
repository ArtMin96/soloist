//! The request/reply protocol between an IPC client (the MCP server) and the app.
//!
//! Each [`IpcRequest`] maps to exactly one `Facade` behaviour; the server pairs it with
//! the connection's identity session for scope. Replies reuse the core read-model types
//! ([`ProcessView`]) so the wire shape can never drift from the domain — except a project
//! is sent as a lean [`ProjectSummary`] (no UI icon blob, which an agent does not need).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use soloist_core::{
    AgentTool, IdentityError, LaunchAgentError, ProcessId, ProcessView, ProjectId, ProjectView,
    ScopedActionError, SpawnAgentError, StartSummary, Whoami,
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
    /// Spawn a configured agent tool as a worker in the session's effective project, by name.
    SpawnAgent {
        tool: String,
        extra_args: Vec<String>,
    },
    /// Every configured agent tool that `spawn_agent` can launch (not scope-filtered).
    ListAgentTools,
    /// Start every trusted command in the session's effective project (trust-gated).
    StartAllCommands,
    /// Gracefully stop every running command in the session's effective project.
    StopAllCommands,
    /// Restart every trusted command in the session's effective project (trust-gated).
    RestartAllCommands,
    /// A process's recent rendered output, bounded to `lines` (server default when omitted).
    GetProcessOutput {
        process: ProcessId,
        lines: Option<usize>,
    },
    /// A process's raw byte output (control sequences included), bounded by the byte cap.
    GetProcessRawOutput { process: ProcessId },
    /// Rendered output lines of a process matching `query`, bounded to `limit`.
    SearchOutput {
        process: ProcessId,
        query: String,
        limit: Option<usize>,
    },
    /// Raw output lines of a process matching `query`, bounded to `limit`.
    SearchRawOutput {
        process: ProcessId,
        query: String,
        limit: Option<usize>,
    },
    /// Clear a process's output buffers (not its PTY), scoped to the session's project.
    ClearOutput { process: ProcessId },
    /// Flush a process's terminal-perf buffer (a no-op in Soloist; confirms the process).
    FlushTerminalPerf { process: ProcessId },
    /// A process's discovered listening ports.
    GetProcessPorts { process: ProcessId },
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
    /// An agent worker was spawned and started; the payload is its new process id.
    Spawned(ProcessId),
    /// Every configured agent tool (answer to [`IpcRequest::ListAgentTools`]).
    AgentTools(Vec<AgentTool>),
    /// A bulk start succeeded; the payload reports what started and what was skipped as
    /// untrusted (answer to [`IpcRequest::StartAllCommands`]).
    BulkStarted(StartSummary),
    /// A bulk stop succeeded; the payload is how many running commands were messaged
    /// (answer to [`IpcRequest::StopAllCommands`]).
    BulkStopped(usize),
    /// Rendered output lines — the answer to a get-output or search request.
    Lines(Vec<String>),
    /// A process's raw byte output, decoded lossily as UTF-8 (control sequences included).
    RawOutput(String),
    /// A process's discovered listening ports (answer to [`IpcRequest::GetProcessPorts`]).
    Ports(Vec<u16>),
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
    /// No agent tool is registered under the requested name.
    #[error("no agent tool is registered under that name")]
    UnknownTool,
    /// The app failed to serve the request (e.g. a durable read failed).
    #[error("the app could not serve the request: {0}")]
    Internal(String),
}

impl IpcError {
    /// Whether the request itself caused the failure — a business-logic refusal or bad
    /// input the caller can act on (unknown target, out of scope, untrusted, no scope in
    /// place) — as opposed to a server-side failure. Each adapter maps the two classes to
    /// its own convention from this one place: an MCP tool returns a request error as a
    /// tool-execution error (`isError: true`) the model can self-correct on, and a server
    /// error as a protocol error; a future HTTP API maps them to 4xx vs 5xx.
    pub fn is_request_error(&self) -> bool {
        match self {
            IpcError::UnknownProcess
            | IpcError::UnknownProject
            | IpcError::NoProjectScope
            | IpcError::OutOfScope
            | IpcError::Untrusted
            | IpcError::UnknownTool => true,
            IpcError::Internal(_) => false,
        }
    }
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

impl From<LaunchAgentError> for IpcError {
    fn from(err: LaunchAgentError) -> Self {
        match err {
            LaunchAgentError::UnknownTool => IpcError::UnknownTool,
            LaunchAgentError::UnknownProject => IpcError::UnknownProject,
            LaunchAgentError::Store(err) => IpcError::Internal(err.to_string()),
            LaunchAgentError::Supervisor(err) => IpcError::Internal(err.to_string()),
        }
    }
}

impl From<SpawnAgentError> for IpcError {
    fn from(err: SpawnAgentError) -> Self {
        match err {
            SpawnAgentError::NoProjectScope => IpcError::NoProjectScope,
            SpawnAgentError::Launch(err) => err.into(),
        }
    }
}

/// A framed reply: success or a typed failure.
pub type IpcResult = Result<IpcResponse, IpcError>;

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
