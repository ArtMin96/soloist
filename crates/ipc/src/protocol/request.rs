//! The request half of the wire protocol: every operation an IPC client can ask of the app.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use soloist_core::{
    IntegrationFile, MissingPolicy, ProcessId, ProjectId, ScratchpadLink, TemplateScope, TimerId,
    TodoDoc, TodoId,
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
    /// Create or replace the scratchpad `name` in the session's effective project with the Markdown
    /// `body`, revision-guarded: `expected_revision` is omitted to create or the current revision to
    /// update.
    ScratchpadWrite {
        name: String,
        body: String,
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
    /// Move the scratchpad `name` from the session's effective project to `to_project` — authorized
    /// only when the caller is authenticated to both (O10).
    ScratchpadTransfer { name: String, to_project: ProjectId },
    /// Create a todo from the disciplined `doc` in the session's effective project, optionally
    /// associated with the scratchpad `scratchpad` names — the document it was derived from.
    TodoCreate {
        doc: TodoDoc,
        #[serde(default)]
        scratchpad: Option<String>,
    },
    /// Every todo in the session's effective project, as one-line summaries.
    TodoList,
    /// One todo by id in the session's effective project.
    TodoGet { todo: TodoId },
    /// Replace the document of `todo` in the session's effective project, revision-guarded by
    /// `expected_revision`, and apply `scratchpad` to its association. Defaulted to
    /// [`ScratchpadLink::Unchanged`], so a caller that says nothing about the association leaves it
    /// standing rather than dropping it along with the replaced document.
    TodoUpdate {
        todo: TodoId,
        doc: TodoDoc,
        #[serde(default)]
        scratchpad: ScratchpadLink<String>,
        expected_revision: u64,
    },
    /// Mark `todo` done in the session's effective project (gated on its blockers).
    TodoComplete { todo: TodoId },
    /// Delete `todo` in the session's effective project.
    TodoDelete { todo: TodoId },
    /// Move `todo` from the session's effective project to `to_project` — authorized only when the
    /// caller is authenticated to both (O10).
    TodoTransfer { todo: TodoId, to_project: ProjectId },
    /// The distinct tags used across the session's effective project's todos.
    TodoTagsList,
    /// Add `tag` to `todo` in the session's effective project.
    TodoAddTag { todo: TodoId, tag: String },
    /// Remove `tag` from `todo` in the session's effective project.
    TodoRemoveTag { todo: TodoId, tag: String },
    /// Replace the blockers of `todo` in the session's effective project.
    TodoSetBlockers { todo: TodoId, blockers: Vec<TodoId> },
    /// Add `blocker` to `todo` in the session's effective project.
    TodoAddBlocker { todo: TodoId, blocker: TodoId },
    /// Remove `blocker` from `todo` in the session's effective project.
    TodoRemoveBlocker { todo: TodoId, blocker: TodoId },
    /// Lock `todo` for the session's bound process (signals, not ownership).
    TodoLock { todo: TodoId },
    /// Release the lock on `todo` if held by the session's bound process.
    TodoUnlock { todo: TodoId },
    /// Add a comment with `body` to `todo` in the session's effective project.
    TodoCommentCreate { todo: TodoId, body: String },
    /// Update comment `comment` of `todo` in the session's effective project.
    TodoCommentUpdate {
        todo: TodoId,
        comment: u64,
        body: String,
    },
    /// Delete comment `comment` of `todo` in the session's effective project.
    TodoCommentDelete { todo: TodoId, comment: u64 },
    /// The comments on `todo` in the session's effective project.
    TodoCommentList { todo: TodoId },
    /// Resolve a `solo://proj/<project>/scratchpad|todo/<id>` link to its content, within the
    /// session's effective project (a foreign-scope or malformed link is refused, not resolved).
    ResolveLink { link: String },
    /// Store `value` at `key` in the session's effective project's kv store (create or replace).
    KvSet {
        key: String,
        value: serde_json::Value,
    },
    /// The value at `key` in the session's effective project's kv store, or `None` if absent.
    KvGet { key: String },
    /// Remove the entry at `key` from the session's effective project's kv store.
    KvDelete { key: String },
    /// Every key-value entry in the session's effective project's kv store, ordered by key.
    KvList,
    /// The MCP feature-group tool enablement — a global settings read (not project-scoped) the MCP
    /// server consults at startup to decide which feature-tool groups to serve.
    McpToolGroups,
    /// Store a feedback message locally (never transmitted anywhere).
    SubmitFeedback { message: String },
    /// The templates the session can address: one scope's when given, else global merged
    /// with the effective project's.
    PromptTemplateList { scope: Option<TemplateScope> },
    /// The template `name` in the chosen scope (project = the session's effective one).
    PromptTemplateRead { scope: TemplateScope, name: String },
    /// Create the template `name` in the chosen scope; a taken name is refused.
    PromptTemplateCreate {
        scope: TemplateScope,
        name: String,
        description: Option<String>,
        body: String,
    },
    /// Replace the template `name`'s description and body, revision-guarded.
    PromptTemplateUpdate {
        scope: TemplateScope,
        name: String,
        description: Option<String>,
        body: String,
        expected_revision: u64,
    },
    /// Delete the template `name` from the chosen scope.
    PromptTemplateDelete { scope: TemplateScope, name: String },
    /// The template `name` as a portable export envelope.
    PromptTemplateExport { scope: TemplateScope, name: String },
    /// The prompt template `name` in the chosen scope, substituted with `values`. `policy` decides
    /// what an unsupplied placeholder means: a caller whose protocol can carry a warning leaves the
    /// marker in the text and reads the gap off the reply, while one that cannot — MCP's
    /// `prompts/get` has no warning channel — refuses the render instead, so a partial prompt is
    /// never mistaken for a complete one.
    PromptTemplateRender {
        scope: TemplateScope,
        name: String,
        values: BTreeMap<String, String>,
        policy: MissingPolicy,
    },
    /// Write the agent guide into the session's effective project root as a managed section.
    SetupAgentIntegration { file: IntegrationFile },
}
