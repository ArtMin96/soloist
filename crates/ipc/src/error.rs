//! The IPC failure taxonomy and the mappings from the core's typed errors to it.
//!
//! Kept separate from the request/reply messages ([`crate::protocol`]) so each file has one
//! purpose. The core contexts surface their own typed errors; this is the single place each is
//! translated to one wire error, and the single place an adapter learns whether a failure was the
//! caller's fault ([`IpcError::is_request_error`]) so it can map the two classes to its own
//! convention (an MCP tool error vs a protocol error; later, an HTTP 4xx vs 5xx).

use serde::{Deserialize, Serialize};
use soloist_core::{
    CoordinationError, FeedbackError, IdentityError, IntegrationWriteError, LaunchAgentError,
    PromptRenderError, RenderError, ScopedActionError, SetupIntegrationError, SpawnAgentError,
    TodoId,
};

/// What an over-cap render is named as in [`IpcError::PayloadTooLarge`], matching how the
/// coordination write caps name the payload they refused.
const RENDERED_PROMPT: &str = "the rendered prompt";

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
    #[error("not bound to a process; bind a session before owning a timer or lease")]
    NoBoundProcess,
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
    #[error("a worker agent cannot spawn agents; report back to the lead that spawned it")]
    WorkerMayNotSpawn,
    /// A feedback submission was refused (empty, oversized, or the store is full); the
    /// detail says why.
    #[error("feedback was not accepted: {0}")]
    InvalidFeedback(String),
    /// The chosen instructions file carries unmatched soloist section markers — replacing a
    /// degenerate span could swallow the user's own content, so the write refused; the
    /// detail names the file to fix by hand.
    #[error("the instructions file was left untouched: {0}")]
    UnmatchedIntegrationMarkers(String),
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
            | IpcError::PayloadTooLarge { .. }
            | IpcError::InvalidScratchpad(_)
            | IpcError::RevisionConflict { .. }
            | IpcError::UnknownScratchpad
            | IpcError::ScratchpadNameTaken
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
            | IpcError::InvalidFeedback(_)
            | IpcError::UnmatchedIntegrationMarkers(_) => true,
            IpcError::Internal(_) => false,
        }
    }
}

impl From<IdentityError> for IpcError {
    fn from(err: IdentityError) -> Self {
        match err {
            IdentityError::UnknownProcess => IpcError::UnknownProcess,
            IdentityError::ForeignProcess => IpcError::ForeignProcess,
            IdentityError::UnknownProject => IpcError::UnknownProject,
            IdentityError::ForeignProject => IpcError::ForeignProject,
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
            SpawnAgentError::WorkerMayNotSpawn => IpcError::WorkerMayNotSpawn,
            SpawnAgentError::Launch(err) => err.into(),
        }
    }
}

impl From<FeedbackError> for IpcError {
    fn from(err: FeedbackError) -> Self {
        match err {
            FeedbackError::Empty | FeedbackError::TooLong | FeedbackError::Full => {
                IpcError::InvalidFeedback(err.to_string())
            }
            FeedbackError::Store(err) => IpcError::Internal(err.to_string()),
        }
    }
}

impl From<SetupIntegrationError> for IpcError {
    fn from(err: SetupIntegrationError) -> Self {
        match err {
            SetupIntegrationError::Scope(err) => err.into(),
            SetupIntegrationError::UnknownProject => IpcError::UnknownProject,
            SetupIntegrationError::Store(err) => IpcError::Internal(err.to_string()),
            SetupIntegrationError::Write(err @ IntegrationWriteError::UnmatchedMarkers { .. }) => {
                IpcError::UnmatchedIntegrationMarkers(err.to_string())
            }
            SetupIntegrationError::Write(err) => IpcError::Internal(err.to_string()),
        }
    }
}

impl From<CoordinationError> for IpcError {
    fn from(err: CoordinationError) -> Self {
        match err {
            CoordinationError::NoProjectScope => IpcError::NoProjectScope,
            CoordinationError::NoBoundProcess => IpcError::NoBoundProcess,
            CoordinationError::InvalidScratchpad(message) => IpcError::InvalidScratchpad(message),
            CoordinationError::RevisionConflict { expected, actual } => {
                IpcError::RevisionConflict { expected, actual }
            }
            CoordinationError::UnknownScratchpad => IpcError::UnknownScratchpad,
            CoordinationError::ScratchpadNameTaken => IpcError::ScratchpadNameTaken,
            CoordinationError::InvalidTodo(message) => IpcError::InvalidTodo(message),
            CoordinationError::TodoRevisionConflict { expected, actual } => {
                IpcError::TodoRevisionConflict { expected, actual }
            }
            CoordinationError::UnknownTodo => IpcError::UnknownTodo,
            CoordinationError::TodoBlocked { by } => IpcError::TodoBlocked { by },
            CoordinationError::UnknownBlocker => IpcError::UnknownBlocker,
            CoordinationError::SelfBlocker => IpcError::SelfBlocker,
            CoordinationError::UnknownComment => IpcError::UnknownComment,
            CoordinationError::InvalidTemplate(message) => IpcError::InvalidTemplate(message),
            CoordinationError::TemplateRevisionConflict { expected, actual } => {
                IpcError::TemplateRevisionConflict { expected, actual }
            }
            CoordinationError::UnknownTemplate => IpcError::UnknownTemplate,
            CoordinationError::TemplateNameTaken => IpcError::TemplateNameTaken,
            CoordinationError::MalformedLink => IpcError::MalformedLink,
            CoordinationError::ForeignScopeLink => IpcError::ForeignScopeLink,
            CoordinationError::ForeignProject => IpcError::ForeignProject,
            CoordinationError::UnknownProject => IpcError::UnknownProject,
            CoordinationError::PayloadTooLarge { what, max_bytes } => IpcError::PayloadTooLarge {
                what: what.to_owned(),
                max_bytes,
            },
            CoordinationError::Store(err) => IpcError::Internal(err.to_string()),
        }
    }
}

impl From<RenderError> for IpcError {
    fn from(err: RenderError) -> Self {
        match err {
            RenderError::TemplateNotFound => IpcError::UnknownTemplate,
            RenderError::RenderedTooLarge { cap, .. } => IpcError::PayloadTooLarge {
                what: RENDERED_PROMPT.to_owned(),
                max_bytes: cap,
            },
            RenderError::MissingValues(names) => IpcError::MissingTemplateValues { names },
            RenderError::Store(err) => IpcError::Internal(err.to_string()),
        }
    }
}

impl From<PromptRenderError> for IpcError {
    fn from(err: PromptRenderError) -> Self {
        match err {
            PromptRenderError::Scope(err) => err.into(),
            PromptRenderError::Render(err) => err.into(),
        }
    }
}
