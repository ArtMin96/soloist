//! The IPC failure taxonomy and the mappings from the core's typed errors to it.
//!
//! Kept separate from the request/reply messages ([`crate::protocol`]) so each file has one
//! purpose. The core contexts surface their own typed errors; this is the single place each is
//! translated to one wire error, and the single place an adapter learns whether a failure was the
//! caller's fault ([`IpcError::is_request_error`]) so it can map the two classes to its own
//! convention (an MCP tool error vs a protocol error; later, an HTTP 4xx vs 5xx).

use serde::{Deserialize, Serialize};
use soloist_core::TodoId;

use crate::vcs_error::GitRefusal;

mod conversions;

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
    /// `bind_session_process` named a process the caller does not run in — the binding is not
    /// authentic. An agent must bind to its own injected `SOLOIST_PROCESS_ID`.
    #[error("that process is not yours to bind")]
    ForeignProcess,
    /// `select_project` named a project the caller does not run in — the scope would not be
    /// authentic. The message carries the remedies, since a caller cannot fix this by retrying.
    #[error(
        "you are not running in that project; scope is proven by the process or the directory you run in — select the project whose directory you are in, keep exactly one project open, or use a global scope where the tool offers one"
    )]
    ForeignProject,
    /// A scoped request was made with no project in scope.
    #[error(
        "no project is in scope; run inside your project's directory (its scope is then automatic), select your own project, keep exactly one project open, or use a global scope where the tool offers one"
    )]
    NoProjectScope,
    /// A coordination action that needs an owning process was made by a session bound to none.
    #[error("not bound to a process; bind a session before owning coordination state or messaging agents")]
    NoBoundProcess,
    /// An agent message named a process outside the caller's live orchestration family.
    #[error("no related agent under that process id")]
    UnknownRecipient,
    /// An agent message named a live process that is not the caller's parent, child, or sibling.
    #[error("that agent is not related to the caller")]
    UnrelatedRecipient,
    /// A message action named one that does not exist in the caller's live-run mailbox.
    #[error("no agent message under that id")]
    UnknownAgentMessage,
    /// A message body exceeds the mailbox's bounded payload size.
    #[error("agent message exceeds the message size limit")]
    AgentMessageTooLarge,
    /// The recipient's bounded pending inbox has no room for another message.
    #[error("the recipient's agent inbox is full")]
    RecipientQueueFull,
    /// The project's bounded live-run mailbox has no room for another message.
    #[error("the project's agent mailbox is full")]
    ProjectQueueFull,
    /// The application's bounded live-run mailbox has no room for another message.
    #[error("the agent mailbox is full")]
    AgentMailboxFull,
    /// The application's bounded aggregate message-body budget has been reached.
    #[error("the agent mailbox byte limit has been reached")]
    AgentMailboxByteLimit,
    /// A coordination write carried a payload larger than its kind allows; `what` names it and
    /// `max_bytes` is the cap it exceeded.
    #[error("{what} exceeds the {max_bytes} byte cap")]
    PayloadTooLarge { what: String, max_bytes: usize },
    /// A scratchpad write carried a malformed document; the detail names every problem.
    #[error("scratchpad is not well-formed: {0}")]
    InvalidScratchpad(String),
    /// A scratchpad write expected a revision other than the one on record — re-read and retry.
    #[error("scratchpad revision conflict (expected {expected:?}, found {actual:?})")]
    RevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A scratchpad action named one that does not exist in the session's effective project.
    #[error("no scratchpad under that name")]
    UnknownScratchpad,
    /// A scratchpad rename targeted a name already used in the project.
    #[error("a scratchpad with that name already exists")]
    ScratchpadNameTaken,
    /// A diagram write carried a malformed source; the detail names every problem.
    #[error("diagram is not well-formed: {0}")]
    InvalidDiagram(String),
    /// A diagram write expected a revision other than the one on record — re-read and retry.
    #[error("diagram revision conflict (expected {expected:?}, found {actual:?})")]
    DiagramRevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A diagram action named one that does not exist in the session's effective project.
    #[error("no diagram under that name")]
    UnknownDiagram,
    /// A diagram rename targeted a name already used in the project.
    #[error("a diagram with that name already exists")]
    DiagramNameTaken,
    /// A todo write carried a malformed document; the detail names every problem.
    #[error("todo is not well-formed: {0}")]
    InvalidTodo(String),
    /// A todo update expected a revision other than the one on record — re-read and retry.
    #[error("todo revision conflict (expected {expected:?}, found {actual:?})")]
    TodoRevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A todo action named one that does not exist in the session's effective project.
    #[error("no todo under that id")]
    UnknownTodo,
    /// Completing a todo was refused because it still has unmet blockers; `by` lists them.
    #[error("todo is blocked by {by:?}")]
    TodoBlocked { by: Vec<TodoId> },
    /// A blocker referenced a todo that does not exist in the session's effective project.
    #[error("no todo under that id to block on")]
    UnknownBlocker,
    /// A todo cannot block itself.
    #[error("a todo cannot block itself")]
    SelfBlocker,
    /// A comment action named one that does not exist on the todo.
    #[error("no comment under that id on that todo")]
    UnknownComment,
    /// A template write carried malformed content; the detail names every problem.
    #[error("template is not well-formed: {0}")]
    InvalidTemplate(String),
    /// A template update expected a revision other than the one on record — re-read and retry.
    #[error("template revision conflict (expected {expected:?}, found {actual:?})")]
    TemplateRevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A template action named one that does not exist in the addressed scope.
    #[error("no template under that name")]
    UnknownTemplate,
    /// A template create named one that already exists in the addressed scope and kind.
    #[error("a template with that name already exists")]
    TemplateNameTaken,
    /// A render that refuses a partial result met placeholders the caller supplied no value for;
    /// `names` lists every one, so they can all be supplied in one retry. Distinct from a generic
    /// failure because a caller can fix it — an adapter maps it to its "bad argument" code.
    #[error("no value supplied for: {}", .names.join(", "))]
    MissingTemplateValues { names: Vec<String> },
    /// A `solo://` link could not be parsed.
    #[error("not a valid solo:// link")]
    MalformedLink,
    /// A `solo://` link named a project other than the caller's effective one — refused, not resolved.
    #[error("that link points outside your effective project")]
    ForeignScopeLink,
    /// The referenced process belongs to a different project than the session's scope.
    #[error("that process belongs to a different project")]
    OutOfScope,
    /// An action targeted a command that is not trusted to run in this project.
    #[error("command is not trusted to run in this project")]
    Untrusted,
    /// No agent tool is registered under the requested name.
    #[error("no agent tool is registered under that name")]
    UnknownTool,
    /// A spawn was requested by a session bound to a process that was itself spawned as a
    /// worker — delegation is one level deep.
    #[error(
        "a worker agent cannot spawn agents or processes; report back to the lead that spawned it"
    )]
    WorkerMayNotSpawn,
    /// A spawn named a command line or label that cannot be run.
    #[error("{0}")]
    InvalidCommand(String),
    /// A feedback submission was refused (empty, oversized, or the store is full); the
    /// detail says why.
    #[error("feedback was not accepted: {0}")]
    InvalidFeedback(String),
    /// The chosen instructions file carries unmatched soloist section markers — replacing a
    /// degenerate span could swallow the user's own content, so the write refused; the
    /// detail names the file to fix by hand.
    #[error("the instructions file was left untouched: {0}")]
    UnmatchedIntegrationMarkers(String),
    /// A version-control call produced no result. `reason` is the closed classification a caller
    /// **matches on** — which is what lets an operation somebody stopped be told from one that
    /// failed — and `message` is the same refusal in words, the core's own sentence plus whatever
    /// account the tool or the service wrote. Classified once, in [`crate::vcs_error`]; nothing
    /// anywhere reads a word version control printed.
    #[error("{message}")]
    Git { reason: GitRefusal, message: String },
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
            | IpcError::ForeignProcess
            | IpcError::ForeignProject
            | IpcError::NoProjectScope
            | IpcError::NoBoundProcess
            | IpcError::UnknownRecipient
            | IpcError::UnrelatedRecipient
            | IpcError::UnknownAgentMessage
            | IpcError::AgentMessageTooLarge
            | IpcError::RecipientQueueFull
            | IpcError::ProjectQueueFull
            | IpcError::AgentMailboxFull
            | IpcError::AgentMailboxByteLimit
            | IpcError::PayloadTooLarge { .. }
            | IpcError::InvalidScratchpad(_)
            | IpcError::RevisionConflict { .. }
            | IpcError::UnknownScratchpad
            | IpcError::ScratchpadNameTaken
            | IpcError::InvalidDiagram(_)
            | IpcError::DiagramRevisionConflict { .. }
            | IpcError::UnknownDiagram
            | IpcError::DiagramNameTaken
            | IpcError::InvalidTodo(_)
            | IpcError::TodoRevisionConflict { .. }
            | IpcError::UnknownTodo
            | IpcError::TodoBlocked { .. }
            | IpcError::UnknownBlocker
            | IpcError::SelfBlocker
            | IpcError::UnknownComment
            | IpcError::InvalidTemplate(_)
            | IpcError::TemplateRevisionConflict { .. }
            | IpcError::UnknownTemplate
            | IpcError::TemplateNameTaken
            | IpcError::MissingTemplateValues { .. }
            | IpcError::MalformedLink
            | IpcError::ForeignScopeLink
            | IpcError::OutOfScope
            | IpcError::Untrusted
            | IpcError::UnknownTool
            | IpcError::WorkerMayNotSpawn
            | IpcError::InvalidCommand(_)
            | IpcError::InvalidFeedback(_)
            | IpcError::UnmatchedIntegrationMarkers(_)
            // Every version-control refusal, without exception: what a caller does about an
            // untrusted project, a credential nobody arranged, a conflict, or an operation the
            // user stopped is to say so — and it can only say so if it was told. A refusal
            // delivered as a protocol error is one the model never sees.
            | IpcError::Git { .. } => true,
            IpcError::Internal(_) => false,
        }
    }
}
