//! The tool parameter structs. Each derives `schemars::JsonSchema`, which is what rmcp
//! turns into a tool's clean-room input schema, and `Deserialize` to receive the call's
//! arguments. They carry no behaviour — the handlers in [`crate::server`] destructure them
//! and forward the values to one IPC request.

use rmcp::schemars;
use serde::Deserialize;
use soloist_core::TodoStatus;

/// Arguments for a single-process tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ProcessArg {
    /// The id of the process, as returned by `list_processes`.
    pub(crate) process: u64,
}

/// Arguments for a project-scoped tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ProjectArg {
    /// The id of the project. Omit to use the session's effective project scope.
    pub(crate) project: Option<u64>,
}

/// Arguments for writing input to a process.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SendInputArg {
    /// The id of the process to write to, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The text to write to the process's input, as UTF-8. Control characters are sent
    /// verbatim — e.g. a trailing carriage return to submit a line, or 0x03 for Ctrl-C.
    pub(crate) input: String,
    /// Optionally wait this many milliseconds after writing, then return the rendered
    /// terminal tail so you can see the effect. Capped by the app; omit to return at once.
    pub(crate) wait_ms: Option<u64>,
}

/// Arguments for renaming a process.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RenameArg {
    /// The id of the process to rename, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The new display label for the process.
    pub(crate) label: String,
}

/// Arguments for spawning a worker agent.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SpawnAgentArg {
    /// The name of a configured agent tool to launch, as listed by `list_agent_tools`.
    pub(crate) tool: String,
    /// Extra command-line flags appended for this one launch ("agent with flags"). Optional.
    #[serde(default)]
    pub(crate) extra_args: Vec<String>,
}

/// Arguments for selecting the session's project scope.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SelectProjectArg {
    /// The id of the project to scope this session's tools to, from `list_projects`.
    pub(crate) project: u64,
}

/// Arguments for registering an external caller.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RegisterAgentArg {
    /// A short label identifying the calling agent (e.g. `claude-code`), reported by `whoami`.
    pub(crate) label: String,
}

/// Arguments for reading a process's rendered output.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct OutputArg {
    /// The id of the process, as returned by `list_processes`.
    pub(crate) process: u64,
    /// How many of the most recent rendered lines to return. Omit for the server default;
    /// the app caps it at the rendered scrollback depth.
    pub(crate) lines: Option<usize>,
}

/// Arguments for searching a process's output.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SearchArg {
    /// The id of the process, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The text to find — a case-sensitive substring. Matching lines are returned in order.
    pub(crate) query: String,
    /// The most matches to return. Omit for the server default; the app caps it.
    pub(crate) limit: Option<usize>,
}

/// Arguments for acquiring a coordination lease.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct LockAcquireArg {
    /// The lease key — a name the coordinating agents agree on (e.g. `deploy`). Project-scoped.
    pub(crate) key: String,
    /// How long to hold the lease before it auto-expires, in milliseconds. Omit for the server
    /// default; re-acquire the same key to renew. The app caps it.
    pub(crate) ttl_ms: Option<u64>,
}

/// Arguments for a lease lookup or release, by key.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct LockKeyArg {
    /// The lease key, scoped to the session's project.
    pub(crate) key: String,
}

/// Arguments for setting a plain timer.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TimerSetArg {
    /// The text delivered to your bound process as a fresh, submitted turn when the timer fires.
    pub(crate) body: String,
    /// Fire this many milliseconds from now. Omit to fire as soon as possible; the app caps it.
    pub(crate) after_ms: Option<u64>,
}

/// Arguments for a fire-when-idle timer.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TimerFireWhenIdleArg {
    /// The text delivered to your bound process as a fresh turn when the watched agents go idle.
    pub(crate) body: String,
    /// The ids of the processes to watch for idle (from `list_processes`) — e.g. workers you spawned.
    pub(crate) processes: Vec<u64>,
    /// A max-wait backstop in milliseconds: fire even if they never go idle. Omit for the app's
    /// default; the app caps it.
    pub(crate) max_wait_ms: Option<u64>,
}

/// Arguments for a timer-management tool, by id.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TimerArg {
    /// The id of the timer, as returned by `timer_set` / `timer_fire_when_idle_*` or `timer_list`.
    pub(crate) timer: u64,
}

/// Arguments for waiting until a process binds a port.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct WaitForPortArg {
    /// The id of the process to watch, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The localhost port to wait for the process to start listening on.
    pub(crate) port: u16,
    /// How long to wait, in milliseconds. Omit for the server default; the app caps it well
    /// under the request timeout, returning `bound: false` if the port has not bound by then.
    /// While waiting, this call holds the session's connection, so other tool calls on the
    /// same session queue behind it until it returns.
    pub(crate) timeout_ms: Option<u64>,
}

/// Arguments naming a single scratchpad by its handle.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadNameArg {
    /// The scratchpad's name handle (unique within the project), as returned by `scratchpad_list`.
    pub(crate) name: String,
}

/// Arguments for writing a scratchpad's disciplined document. The fields ARE the required structure
/// — every scratchpad records the same sections, so they stay consistent and informative.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadWriteArg {
    /// The scratchpad's name handle (unique within the project). Omit `expected_revision` to create
    /// it; pass the current revision (from `scratchpad_read`) to update it.
    pub(crate) name: String,
    /// What this scratchpad is for — the goal it serves, in a sentence or two.
    pub(crate) objective: String,
    /// The background and current state a reader needs to act on it.
    pub(crate) context: String,
    /// The ordered path to the objective: each entry one step, in order. At least one.
    pub(crate) plan: Vec<String>,
    /// The testable criteria that define the objective as done. At least one.
    pub(crate) acceptance_criteria: Vec<String>,
    /// The risks, unknowns, or blockers to watch. State "none identified" rather than leaving empty.
    pub(crate) risks: Vec<String>,
    /// Where the work stands right now.
    pub(crate) status: String,
    /// Anything the structured sections do not cover — free Markdown. Optional.
    pub(crate) notes: Option<String>,
    /// The revision you are updating from, as returned by `scratchpad_read`. Omit to create a new
    /// scratchpad; a mismatch means someone edited it first, so re-read and retry.
    pub(crate) expected_revision: Option<u64>,
}

/// Arguments for renaming a scratchpad.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadRenameArg {
    /// The scratchpad's current name handle.
    pub(crate) name: String,
    /// The new name handle (must be unused in the project).
    pub(crate) new_name: String,
}

/// Arguments for adding or removing a scratchpad's tags.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadTagsArg {
    /// The scratchpad's name handle.
    pub(crate) name: String,
    /// The tags to add or remove.
    pub(crate) tags: Vec<String>,
}

/// Arguments for archiving or restoring a scratchpad.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadArchiveArg {
    /// The scratchpad's name handle.
    pub(crate) name: String,
    /// True to archive it (hide from the default listing), false to restore it.
    pub(crate) archived: bool,
}

/// The lifecycle status an agent declares on a todo — a closed set, mirroring the core
/// `TodoStatus` on the wire; the handler converts it. Distinct from the *blocker gate*: a todo is
/// prevented from completing by its unmet blockers, not by this label.
#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TodoStatusArg {
    Open,
    Blocked,
    InProgress,
    Done,
}

impl From<TodoStatusArg> for TodoStatus {
    fn from(status: TodoStatusArg) -> Self {
        match status {
            TodoStatusArg::Open => TodoStatus::Open,
            TodoStatusArg::Blocked => TodoStatus::Blocked,
            TodoStatusArg::InProgress => TodoStatus::InProgress,
            TodoStatusArg::Done => TodoStatus::Done,
        }
    }
}

/// Arguments naming a single todo by id.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoArg {
    /// The id of the todo, as returned by `todo_list` or `todo_create`.
    pub(crate) todo: u64,
}

/// Arguments for creating a todo's disciplined document. The fields ARE the required structure —
/// every todo records the same sections, so they stay consistent and informative.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoCreateArg {
    /// A short imperative title — what this todo is.
    pub(crate) title: String,
    /// What needs doing and any detail a worker needs to act on it.
    pub(crate) description: String,
    /// The testable criteria that define the todo as done. At least one.
    pub(crate) acceptance_criteria: Vec<String>,
    /// The risks, unknowns, or blockers to watch. State "none identified" rather than leaving empty.
    pub(crate) risks: Vec<String>,
    /// The lifecycle status to start in (usually open).
    pub(crate) status: TodoStatusArg,
}

/// Arguments for updating a todo's document, revision-guarded.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoUpdateArg {
    /// The id of the todo to update.
    pub(crate) todo: u64,
    /// A short imperative title — what this todo is.
    pub(crate) title: String,
    /// What needs doing and any detail a worker needs to act on it.
    pub(crate) description: String,
    /// The testable criteria that define the todo as done. At least one.
    pub(crate) acceptance_criteria: Vec<String>,
    /// The risks, unknowns, or blockers to watch. State "none identified" rather than leaving empty.
    pub(crate) risks: Vec<String>,
    /// The lifecycle status. Set it to done only when the todo's blockers are all complete.
    pub(crate) status: TodoStatusArg,
    /// The revision you are updating from, as returned by `todo_get`. A mismatch means someone
    /// edited it first, so re-read and retry.
    pub(crate) expected_revision: u64,
}

/// Arguments for adding or removing a single tag on a todo.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoTagArg {
    /// The id of the todo.
    pub(crate) todo: u64,
    /// The tag to add or remove.
    pub(crate) tag: String,
}

/// Arguments for replacing a todo's blockers.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoBlockersArg {
    /// The id of the todo.
    pub(crate) todo: u64,
    /// The ids of the todos that must complete before this one (from `todo_list`).
    pub(crate) blockers: Vec<u64>,
}

/// Arguments for adding or removing a single blocker on a todo.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoBlockerArg {
    /// The id of the todo to gate.
    pub(crate) todo: u64,
    /// The id of the todo that must complete first.
    pub(crate) blocker: u64,
}

/// Arguments for creating a comment on a todo.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoCommentCreateArg {
    /// The id of the todo to comment on.
    pub(crate) todo: u64,
    /// The comment text.
    pub(crate) body: String,
}

/// Arguments for updating a comment on a todo.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoCommentEditArg {
    /// The id of the todo.
    pub(crate) todo: u64,
    /// The id of the comment, as returned by `todo_comment_create` or seen on the todo.
    pub(crate) comment: u64,
    /// The new comment text.
    pub(crate) body: String,
}

/// Arguments for referencing a comment on a todo (delete).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoCommentRefArg {
    /// The id of the todo.
    pub(crate) todo: u64,
    /// The id of the comment to delete.
    pub(crate) comment: u64,
}

/// Arguments for storing a kv entry.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct KvSetArg {
    /// The key to store the value at. Case-sensitive; unique within the project.
    pub(crate) key: String,
    /// The JSON value to store. Can be any valid JSON — an object, array, string, number, or
    /// boolean. Replaces the previous value if the key already exists.
    pub(crate) value: serde_json::Value,
}

/// Arguments for reading or deleting a kv entry by key.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct KvKeyArg {
    /// The key to read or delete.
    pub(crate) key: String,
}
