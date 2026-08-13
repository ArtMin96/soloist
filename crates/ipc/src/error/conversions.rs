use soloist_core::{
    AgentMailboxError, CoordinationError, FeedbackError, IdentityError, IntegrationWriteError,
    LaunchAgentError, PromptRenderError, RenderError, ScopedActionError, SetupIntegrationError,
    SpawnAgentError,
};

use super::IpcError;

const RENDERED_PROMPT: &str = "the rendered prompt";

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
            SpawnAgentError::Mailbox(err) => err.into(),
        }
    }
}

impl From<AgentMailboxError> for IpcError {
    fn from(err: AgentMailboxError) -> Self {
        match err {
            AgentMailboxError::NoProjectScope => IpcError::NoProjectScope,
            AgentMailboxError::NoBoundProcess => IpcError::NoBoundProcess,
            AgentMailboxError::UnknownRecipient => IpcError::UnknownRecipient,
            AgentMailboxError::UnrelatedRecipient => IpcError::UnrelatedRecipient,
            AgentMailboxError::UnknownMessage => IpcError::UnknownAgentMessage,
            AgentMailboxError::MessageTooLarge => IpcError::AgentMessageTooLarge,
            AgentMailboxError::RecipientQueueFull => IpcError::RecipientQueueFull,
            AgentMailboxError::ProjectQueueFull => IpcError::ProjectQueueFull,
            AgentMailboxError::GlobalQueueFull => IpcError::AgentMailboxFull,
            AgentMailboxError::GlobalByteLimit => IpcError::AgentMailboxByteLimit,
            AgentMailboxError::UnknownTodo => IpcError::UnknownTodo,
            AgentMailboxError::TodoBlocked { by } => IpcError::TodoBlocked { by },
            AgentMailboxError::Store(err) => IpcError::Internal(err.to_string()),
            AgentMailboxError::Supervisor(err) => IpcError::Internal(err.to_string()),
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
            CoordinationError::InvalidDiagram(message) => IpcError::InvalidDiagram(message),
            CoordinationError::DiagramRevisionConflict { expected, actual } => {
                IpcError::DiagramRevisionConflict { expected, actual }
            }
            CoordinationError::UnknownDiagram => IpcError::UnknownDiagram,
            CoordinationError::DiagramNameTaken => IpcError::DiagramNameTaken,
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
