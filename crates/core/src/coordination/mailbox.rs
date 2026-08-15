//! The ephemeral, bounded addressed mailbox shared by live agents.

mod completion_notice;
mod onboarding;
mod reactor;
mod state;
mod transcript;
mod vocabulary;

pub(crate) use completion_notice::CompletionNoticeState;
pub use onboarding::{orchestration_guide, OrchestrationGuide};
pub use reactor::AgentMailboxReactor;
pub use state::AgentMailbox;
pub use vocabulary::{
    AgentBroadcastReceipt, AgentMessage, AgentMessageDelivery, AgentMessageKind,
    AgentMessageOutcome, AgentMessageReceipt, AgentMessageRecord, AgentRelationship,
    AgentRosterEntry, MailboxCapacityError, MAX_AGENT_MESSAGE_BYTES, MAX_PENDING_AGENT_MESSAGES,
    MAX_PENDING_AGENT_MESSAGE_BYTES, MAX_PENDING_MESSAGES_PER_PROJECT,
    MAX_PENDING_MESSAGES_PER_RECIPIENT, MAX_TRANSCRIPT_BODY_BYTES, MAX_TRANSCRIPT_ENTRIES,
    MAX_TRANSCRIPT_ENTRIES_PER_PROJECT,
};
