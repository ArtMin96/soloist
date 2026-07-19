//! The reply half of the wire protocol: the success variants and the lean agent-facing
//! projections ([`ProjectSummary`], [`ProjectStatus`]).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use soloist_core::{
    AcquireOutcome, AgentTool, Comment, ExportedTemplate, FeedbackEntry, IntegrationWrite, KvEntry,
    LeaseView, LinkContent, McpToolGroups, ProcessId, ProcessView, ProjectId, ProjectView,
    ScratchpadSummary, ScratchpadView, SetWhenIdleOutcome, StartSummary, TemplateSummary,
    TemplateView, TimerView, TodoSummary, TodoView, Whoami,
};

use crate::error::IpcError;

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
    /// One scratchpad (answer to a read, rename, tag, or archive request). Reuses the core view so
    /// the wire shape — including the canonically rendered Markdown — cannot drift.
    Scratchpad(ScratchpadView),
    /// A written scratchpad plus the template that seeded it (answer to [`IpcRequest::ScratchpadWrite`]).
    /// `seeded_from` names the default template whose body seeded an empty create, or `None` on an
    /// update or when nothing seeded.
    ScratchpadWritten {
        scratchpad: ScratchpadView,
        seeded_from: Option<String>,
    },
    /// Every scratchpad in scope, as one-line summaries (answer to [`IpcRequest::ScratchpadList`]).
    Scratchpads(Vec<ScratchpadSummary>),
    /// The distinct scratchpad tags in scope (answer to [`IpcRequest::ScratchpadTagsList`]).
    ScratchpadTags(Vec<String>),
    /// Whether a scratchpad was deleted (answer to [`IpcRequest::ScratchpadDelete`]).
    ScratchpadDeleted(bool),
    /// One todo (answer to a get, update, tag, blocker, or lock request). Reuses the core view so
    /// the wire shape cannot drift.
    Todo(TodoView),
    /// A created todo plus the template that seeded it (answer to [`IpcRequest::TodoCreate`]).
    /// `seeded_from` names the default template whose body seeded an empty body, or `None`.
    TodoCreated {
        todo: TodoView,
        seeded_from: Option<String>,
    },
    /// Every todo in scope, as one-line summaries (answer to [`IpcRequest::TodoList`]).
    Todos(Vec<TodoSummary>),
    /// A todo and a new comment's id (answer to [`IpcRequest::TodoCommentCreate`]).
    TodoComment { todo: TodoView, comment: u64 },
    /// The comments on a todo (answer to [`IpcRequest::TodoCommentList`]).
    TodoComments(Vec<Comment>),
    /// The content a `solo://` link resolved to (answer to [`IpcRequest::ResolveLink`]) — the
    /// in-scope scratchpad or todo it points to. Reuses the core view so the wire shape cannot drift.
    Link(LinkContent),
    /// The distinct todo tags in scope (answer to [`IpcRequest::TodoTagsList`]).
    TodoTags(Vec<String>),
    /// Whether a todo was deleted (answer to [`IpcRequest::TodoDelete`]).
    TodoDeleted(bool),
    /// The value at a kv key, or `None` if absent (answer to [`IpcRequest::KvGet`] and
    /// [`IpcRequest::KvSet`]).
    KvValue(Option<serde_json::Value>),
    /// Every key-value entry in scope (answer to [`IpcRequest::KvList`]). Reuses the core entry
    /// type so the wire shape cannot drift.
    KvPairs(Vec<KvEntry>),
    /// Whether a kv entry was deleted (answer to [`IpcRequest::KvDelete`]).
    KvDeleted(bool),
    /// The MCP feature-group tool enablement (answer to [`IpcRequest::McpToolGroups`]). Reuses the
    /// core type so the wire shape cannot drift.
    McpToolGroups(McpToolGroups),
    /// A stored feedback entry (answer to [`IpcRequest::SubmitFeedback`]). Reuses the core type so
    /// the wire shape cannot drift.
    Feedback(FeedbackEntry),
    /// One prompt template (answer to a read, create, or update request). Reuses the core view so
    /// the wire shape — including the kind and derived placeholders — cannot drift.
    PromptTemplate(TemplateView),
    /// The templates in scope, as summaries (answer to [`IpcRequest::PromptTemplateList`]).
    PromptTemplates(Vec<TemplateSummary>),
    /// Whether a template was deleted (answer to [`IpcRequest::PromptTemplateDelete`]).
    PromptTemplateDeleted(bool),
    /// A template's portable export envelope (answer to [`IpcRequest::PromptTemplateExport`]).
    PromptTemplateExport(ExportedTemplate),
    /// What a guide write did (answer to [`IpcRequest::SetupAgentIntegration`]). Reuses the core
    /// type so the wire shape cannot drift.
    IntegrationWritten(IntegrationWrite),
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
