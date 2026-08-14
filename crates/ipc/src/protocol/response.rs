//! The reply half of the wire protocol: the success variants and the lean agent-facing
//! projections ([`ProjectSummary`], [`ProjectStatus`]).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use soloist_core::{
    AcquireOutcome, AgentBroadcastReceipt, AgentMessageDelivery, AgentMessageId,
    AgentMessageOutcome, AgentRosterEntry, AgentTool, Branches, Comment, CompletionReport,
    DiagramSummary, DiagramView, ExportedTemplate, FeedbackEntry, FileDiff, GitStatus,
    IntegrationWrite, KvEntry, LeaseView, LinkContent, McpToolGroups, ProcessId, ProcessView,
    ProjectId, ProjectView, PullRequestReview, PullRequestSurface, RenderedPrompt,
    ScratchpadSummary, ScratchpadView, SeedTemplate, SetWhenIdleOutcome, StartSummary,
    TemplateSummary, TemplateView, TimerView, TodoSummary, TodoView, TrustRequestOutcome,
    TrustRequestState, Whoami,
};

use crate::error::IpcError;
use crate::frame::MAX_FRAME;

/// A conservative serialized upper bound for one compact broadcast receipt row: two `u64` ids,
/// the longest delivery tag, field names, punctuation, and slack for JSON framing.
const MAX_BROADCAST_RECEIPT_ROW_BYTES: usize = 128;
const MAX_BROADCAST_RECEIPT_ROWS: usize = soloist_core::MAX_PENDING_MESSAGES_PER_PROJECT;
const MAX_BROADCAST_RECEIPT_ENVELOPE_BYTES: usize = 256;
const _: () = assert!(
    MAX_BROADCAST_RECEIPT_ROW_BYTES * MAX_BROADCAST_RECEIPT_ROWS
        + MAX_BROADCAST_RECEIPT_ENVELOPE_BYTES
        < MAX_FRAME as usize
);

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
    /// An agent worker was spawned and started without initial-message metadata. Kept as the exact
    /// legacy response for promptless spawns.
    Spawned(ProcessId),
    /// An agent worker was spawned and an initial onboarding/task message was queued for it.
    SpawnedWithMessage {
        process: ProcessId,
        initial_message_id: AgentMessageId,
        delivery: AgentMessageOutcome,
    },
    /// A trust request was recorded, or the variant was already trusted so there was nothing to
    /// ask (answer to [`IpcRequest::RequestCommandTrust`]).
    TrustRequestOpened(TrustRequestOutcome),
    /// Where a trust request stands (answer to [`IpcRequest::TrustRequestStatus`]).
    TrustRequest(TrustRequestState),
    /// Every configured agent tool (answer to [`IpcRequest::ListAgentTools`]).
    AgentTools(Vec<AgentTool>),
    /// The caller and its related live agents (answer to [`IpcRequest::AgentRoster`]).
    AgentRoster(Vec<AgentRosterEntry>),
    /// One message delivery (answer to send or acknowledge).
    AgentMessageDelivery(AgentMessageDelivery),
    /// Compact receipts created by a lineage-group broadcast. Bodies are deliberately omitted so
    /// the bounded response remains far below the IPC frame cap even at the project queue limit.
    AgentMessageBroadcast(AgentBroadcastReceipt),
    /// One message from the caller's inbox, with the delivery state it currently holds.
    AgentMessage(AgentMessageDelivery),
    /// The caller's pending, unacknowledged inbox, each message with its delivery state.
    AgentMessages(Vec<AgentMessageDelivery>),
    /// A durable completion and the queued, pending, or deferred notification state.
    AgentCompletion(CompletionReport),
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
    /// One diagram (answer to a write, read, rename, tag, or archive request). Reuses the core view
    /// so the wire shape — including the raw Mermaid source — cannot drift.
    Diagram(DiagramView),
    /// Every diagram in scope, as one-line summaries (answer to [`IpcRequest::DiagramList`]).
    Diagrams(Vec<DiagramSummary>),
    /// The distinct diagram tags in scope (answer to [`IpcRequest::DiagramTagsList`]).
    DiagramTags(Vec<String>),
    /// Whether a diagram was deleted (answer to [`IpcRequest::DiagramDelete`]).
    DiagramDeleted(bool),
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
    /// A repository's working-tree status (answer to [`IpcRequest::GitStatus`]). Reuses the core
    /// read model so the wire shape cannot drift.
    GitStatus(GitStatus),
    /// One path's diff, or `None` where the path names nothing inside the repository (answer to
    /// [`IpcRequest::GitDiff`]).
    GitDiff(Option<FileDiff>),
    /// The branches a switcher can offer, and whether anything is stashed (answer to
    /// [`IpcRequest::GitBranches`]).
    GitBranches(Branches),
    /// What can be proposed as a pull request, and what already has been (answer to
    /// [`IpcRequest::GitPullRequest`]).
    GitPullRequest(PullRequestSurface),
    /// An open pull request with its checks and conversations, or `None` when the branch has none
    /// (answer to [`IpcRequest::GitPullRequestReview`]).
    GitPullRequestReview(Option<PullRequestReview>),
    /// The address of a pull request that was just made (answer to
    /// [`IpcRequest::GitCreatePullRequest`]).
    GitPullRequestCreated(String),
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
    /// A rendered prompt (answer to [`IpcRequest::PromptTemplateRender`]). Reuses the core result so
    /// the wire shape — the text, the target it was rendered for, and both advisory reports —
    /// cannot drift.
    PromptTemplateRendered(RenderedPrompt),
    /// The template a new document would be seeded from, or `None` when the local user has selected
    /// no default for that kind (answer to [`IpcRequest::SeedTemplateRead`]). Reuses the core seed
    /// projection — the body and the name a create applies, not the template's authoring metadata —
    /// so the wire shape cannot drift from what seeding does.
    SeedTemplate(Option<SeedTemplate>),
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

/// One remark an operation made about itself while it was still running.
///
/// Prose from a program Soloist does not own, carried verbatim for whoever asked to be told. Nothing
/// reads it to decide anything — it exists to tell a waiting caller that work is happening and
/// roughly where it has got to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressReport {
    /// The latest thing the operation said, already coalesced and rate-limited at its source.
    pub note: String,
}

/// What one frame travelling back from the app is: a remark about a request still in flight, or the
/// answer that ends it.
///
/// Untagged, and deliberately: [`IpcResult`] serializes as it always has, so every reply to every
/// request that never asked for progress is byte for byte the frame it was before this existed. The
/// two are told apart by shape — a result is keyed `Ok` or `Err`, a remark is keyed `note` — so
/// neither can be read as the other.
///
/// A request that asked for progress may be preceded by any number of remarks, and is always ended
/// by exactly one answer; a request that did not is answered by one frame, as before.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcReply {
    /// The operation is still running, and this is the latest thing it said.
    Progress(ProgressReport),
    /// The operation ended, and this is its answer. Boxed because an answer is many times the size
    /// of a remark, and a frame is built once and written once — so the indirection costs one
    /// allocation on a path that already serializes, and saves carrying an answer's worth of stack
    /// for every remark.
    Done(Box<IpcResult>),
}
