//! Parameter structs for the coordination tools: leases, timers, scratchpads, todos, and
//! the key-value store.

use rmcp::schemars;
use serde::Deserialize;
use soloist_core::TodoStatus;

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

/// Arguments naming a single scratchpad by its handle.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadNameArg {
    /// The scratchpad's name handle (unique within the project), as returned by `scratchpad_list`.
    pub(crate) name: String,
}

/// Arguments for writing a scratchpad's free-form Markdown body.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadWriteArg {
    /// The scratchpad's name handle (unique within the project). Omit `expected_revision` to create
    /// it; pass the current revision (from `scratchpad_read`) to update it.
    pub(crate) name: String,
    /// The scratchpad's Markdown content — the whole body. Do not repeat the name as a heading; it
    /// is the handle, not part of the body. May be empty.
    pub(crate) content: String,
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

/// Arguments for transferring a scratchpad to another project.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ScratchpadTransferArg {
    /// The scratchpad's name handle in your effective project.
    pub(crate) name: String,
    /// The id of the destination project — you must be authenticated to it (a process you run in
    /// belongs to it), or the transfer is refused.
    pub(crate) to_project: u64,
}

/// Arguments naming a single diagram by its handle.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct DiagramNameArg {
    /// The diagram's name handle (unique within the project), as returned by `diagram_list`.
    pub(crate) name: String,
}

/// Arguments for writing a diagram's Mermaid source.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct DiagramWriteArg {
    /// The diagram's name handle (unique within the project). Omit `expected_revision` to create it;
    /// pass the current revision (from `diagram_read`) to update it.
    pub(crate) name: String,
    /// The diagram's Mermaid source — the whole diagram definition (e.g. a `flowchart`, `sequenceDiagram`,
    /// or `classDiagram` block). Stored verbatim; Soloist does not render or validate it. May be empty.
    pub(crate) source: String,
    /// The revision you are updating from, as returned by `diagram_read`. Omit to create a new
    /// diagram; a mismatch means someone edited it first, so re-read and retry.
    pub(crate) expected_revision: Option<u64>,
}

/// Arguments for renaming a diagram.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct DiagramRenameArg {
    /// The diagram's current name handle.
    pub(crate) name: String,
    /// The new name handle (must be unused in the project).
    pub(crate) new_name: String,
}

/// Arguments for adding or removing a diagram's tags.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct DiagramTagsArg {
    /// The diagram's name handle.
    pub(crate) name: String,
    /// The tags to add or remove.
    pub(crate) tags: Vec<String>,
}

/// Arguments for archiving or restoring a diagram.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct DiagramArchiveArg {
    /// The diagram's name handle.
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

/// Arguments for transferring a todo to another project.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoTransferArg {
    /// The id of the todo in your effective project, as returned by `todo_list`.
    pub(crate) todo: u64,
    /// The id of the destination project — you must be authenticated to it (a process you run in
    /// belongs to it), or the transfer is refused.
    pub(crate) to_project: u64,
}

/// A reference to a todo for `todo_get`: either its numeric id or a `solo://` link someone handed
/// you. A number is read as the id; a string is read as a link.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum TodoRef {
    Id(u64),
    Link(String),
}

/// Arguments for reading one todo, by its id or a `solo://` link to it.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoGetArg {
    /// The todo to read: its numeric id (from `todo_list`/`todo_create`) or a `solo://` link to it.
    pub(crate) todo: TodoRef,
}

/// Arguments for creating a todo.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoCreateArg {
    /// A short imperative title — what this todo is.
    pub(crate) title: String,
    /// The free-form Markdown body: what needs doing and any detail a worker needs. Optional — omit
    /// for an empty body.
    pub(crate) body: Option<String>,
    /// The lifecycle status to start in; defaults to open when omitted.
    pub(crate) status: Option<TodoStatusArg>,
    /// The name handle of the scratchpad this todo derives from — set it only when the todo came
    /// out of that document (for example a task you extracted from its plan). Otherwise omit it:
    /// having no scratchpad is normal and permanent, never something to fill in. An unknown name is
    /// refused and nothing is created.
    pub(crate) scratchpad: Option<String>,
}

/// Arguments for updating a todo, revision-guarded. The whole document is replaced, so provide the
/// title and status you want; the body is optional (omit to clear it).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct TodoUpdateArg {
    /// The id of the todo to update.
    pub(crate) todo: u64,
    /// A short imperative title — what this todo is.
    pub(crate) title: String,
    /// The free-form Markdown body: what needs doing and any detail a worker needs. Optional — omit
    /// for an empty body.
    pub(crate) body: Option<String>,
    /// The lifecycle status. Set it to done only when the todo's blockers are all complete.
    pub(crate) status: TodoStatusArg,
    /// The name handle of the scratchpad this todo derives from. Unlike `body`, **omitting this
    /// leaves the existing link exactly as it is** — the link is coordination state alongside the
    /// todo's tags and blockers, not part of the document this call replaces, so a routine title or
    /// status edit never destroys it. Pass a name to (re)link, or an explicit `null` to unlink. An
    /// unknown name is refused and nothing is written.
    #[serde(default, deserialize_with = "stated_option")]
    #[schemars(with = "Option<String>")]
    pub(crate) scratchpad: Option<Option<String>>,
    /// The revision you are updating from, as returned by `todo_get`. A mismatch means someone
    /// edited it first, so re-read and retry.
    pub(crate) expected_revision: u64,
}

/// Distinguishes a field the caller omitted from one they explicitly set to `null`, which serde's
/// own `Option` handling collapses into the same `None`. The outer layer is "did they say
/// anything", the inner "what did they say" — the shape a three-state link needs on the wire.
fn stated_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::deserialize(deserializer).map(Some)
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
