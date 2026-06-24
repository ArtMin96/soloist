//! The request/reply protocol between an IPC client (the MCP server) and the app.
//!
//! Each [`IpcRequest`] maps to exactly one `Facade` behaviour; the server pairs it with
//! the connection's identity session for scope. Replies reuse the core read-model types
//! ([`ProcessView`]) so the wire shape can never drift from the domain — except a project
//! is sent as a lean [`ProjectSummary`] (no UI icon blob, which an agent does not need).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use soloist_core::{
    AcquireOutcome, AgentTool, LeaseView, ProcessId, ProcessView, ProjectId, ProjectView,
    ScratchpadDoc, ScratchpadSummary, ScratchpadView, SetWhenIdleOutcome, StartSummary, TimerId,
    TimerView, Whoami,
};

use crate::error::IpcError;

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
    /// Set this session's informational selected-process hint (reported by `whoami`).
    SelectProcess { process: ProcessId },
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
    /// Rename one process's display label, scoped to the session's effective project.
    RenameProcess { process: ProcessId, label: String },
    /// Stop and remove one process from the registry, scoped to the session's effective project.
    CloseProcess { process: ProcessId },
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
    /// The command processes (services) of the session's effective project.
    ServicesList,
    /// Wait until a process binds `port`, or `timeout_ms` elapses (server-bounded).
    WaitForBoundPort {
        process: ProcessId,
        port: u16,
        timeout_ms: Option<u64>,
    },
    /// Acquire the lease `key` in the session's effective project, owned by its bound process.
    /// `ttl_ms` is the lease lifetime; omit it for the core's default (the default and the bounds
    /// live in the core, so every frontend shares them). Non-blocking: a held key reports its holder.
    LockAcquire { key: String, ttl_ms: Option<u64> },
    /// The current holder of the lease `key` in the session's effective project, if any.
    LockStatus { key: String },
    /// Release the lease `key` if held by the session's bound process.
    LockRelease { key: String },
    /// Arm a timer owned by the session's bound process that delivers `body` to it as a fresh
    /// turn after `after_ms` (immediately when omitted). The default/ceiling live in the core.
    TimerSet { body: String, after_ms: Option<u64> },
    /// Arm a timer that delivers `body` when **any** of `processes` is idle, or `max_wait_ms`
    /// elapses (the core's default backstop when omitted).
    TimerFireWhenIdleAny {
        body: String,
        processes: Vec<ProcessId>,
        max_wait_ms: Option<u64>,
    },
    /// Arm a timer that delivers `body` when **every** one of `processes` is idle, or
    /// `max_wait_ms` elapses (the core's default backstop when omitted).
    TimerFireWhenIdleAll {
        body: String,
        processes: Vec<ProcessId>,
        max_wait_ms: Option<u64>,
    },
    /// Cancel a timer the session's bound process owns.
    TimerCancel { timer: TimerId },
    /// Pause a timer the session's bound process owns.
    TimerPause { timer: TimerId },
    /// Resume a paused timer the session's bound process owns.
    TimerResume { timer: TimerId },
    /// Every timer the session's bound process owns.
    TimerList,
    /// Create or replace the scratchpad `name` in the session's effective project with the
    /// disciplined `doc`, revision-guarded: `expected_revision` is omitted to create or the current
    /// revision to update.
    ScratchpadWrite {
        name: String,
        doc: ScratchpadDoc,
        expected_revision: Option<u64>,
    },
    /// The scratchpad `name` in the session's effective project.
    ScratchpadRead { name: String },
    /// Every scratchpad in the session's effective project, as one-line summaries.
    ScratchpadList,
    /// Rename the scratchpad `name` to `new_name` in the session's effective project.
    ScratchpadRename { name: String, new_name: String },
    /// Add `tags` to the scratchpad `name` in the session's effective project.
    ScratchpadAddTags { name: String, tags: Vec<String> },
    /// Remove `tags` from the scratchpad `name` in the session's effective project.
    ScratchpadRemoveTags { name: String, tags: Vec<String> },
    /// The distinct tags used across the session's effective project's scratchpads.
    ScratchpadTagsList,
    /// Archive or restore the scratchpad `name` in the session's effective project.
    ScratchpadArchive { name: String, archived: bool },
    /// Delete the scratchpad `name` in the session's effective project.
    ScratchpadDelete { name: String },
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
    /// The outcome of a port-readiness wait (answer to [`IpcRequest::WaitForBoundPort`]).
    PortWait(PortWaitOutcome),
    /// The outcome of a lease acquire — granted or held by another (answer to
    /// [`IpcRequest::LockAcquire`]). Reuses the core type so the wire shape cannot drift.
    LeaseOutcome(AcquireOutcome),
    /// The current holder of a lease, or `None` if free (answer to [`IpcRequest::LockStatus`]).
    LeaseStatus(Option<LeaseView>),
    /// Whether the caller's lease was released (answer to [`IpcRequest::LockRelease`]).
    LeaseReleased(bool),
    /// A timer was armed (answer to [`IpcRequest::TimerSet`]). Reuses the core view so the wire
    /// shape cannot drift.
    TimerArmed(TimerView),
    /// A fire-when-idle timer was armed, with whether its condition is already met and which
    /// processes it is still waiting on (answer to the `TimerFireWhenIdle*` requests).
    TimerWhenIdle(SetWhenIdleOutcome),
    /// Whether a timer-management action affected a timer (answer to [`IpcRequest::TimerCancel`],
    /// [`IpcRequest::TimerPause`], and [`IpcRequest::TimerResume`]).
    TimerChanged(bool),
    /// Every timer the caller owns (answer to [`IpcRequest::TimerList`]).
    Timers(Vec<TimerView>),
    /// One scratchpad (answer to a read, write, rename, tag, or archive request). Reuses the core
    /// view so the wire shape — including the canonically rendered Markdown — cannot drift.
    Scratchpad(ScratchpadView),
    /// Every scratchpad in scope, as one-line summaries (answer to [`IpcRequest::ScratchpadList`]).
    Scratchpads(Vec<ScratchpadSummary>),
    /// The distinct scratchpad tags in scope (answer to [`IpcRequest::ScratchpadTagsList`]).
    ScratchpadTags(Vec<String>),
    /// Whether a scratchpad was deleted (answer to [`IpcRequest::ScratchpadDelete`]).
    ScratchpadDeleted(bool),
}

/// How a [`IpcRequest::WaitForBoundPort`] resolved — a structured answer, not an error: a
/// timeout is the wait reporting "not bound yet", which the caller can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortWaitOutcome {
    /// The port is bound and the process now reads ready.
    Bound,
    /// The port did not bind within the (bounded) timeout.
    TimedOut,
    /// The process is not running, so it has no group that could bind a port.
    NotRunning,
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

/// A framed reply: success or a typed failure. The failure taxonomy and its mappings from the
/// core's errors live in [`crate::error`].
pub type IpcResult = Result<IpcResponse, IpcError>;

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
