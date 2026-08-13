use serde::{Deserialize, Serialize};

use crate::ids::{AgentMessageId, ProcessId, ProjectId, TodoId};
use crate::process::ProcStatus;

/// Maximum UTF-8 bytes in one addressed message.
pub const MAX_AGENT_MESSAGE_BYTES: usize = 16 * 1024;
/// Maximum pending messages held for one recipient.
pub const MAX_PENDING_MESSAGES_PER_RECIPIENT: usize = 64;
/// Maximum pending messages held across one project.
pub const MAX_PENDING_MESSAGES_PER_PROJECT: usize = 1024;
/// Maximum pending messages held across the entire running application.
pub const MAX_PENDING_AGENT_MESSAGES: usize = 4096;
/// Maximum UTF-8 body bytes held across the entire running application.
pub const MAX_PENDING_AGENT_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Why an addressed message exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageKind {
    Direct,
    Task,
    Completion,
}

/// The observable delivery state of an addressed message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageOutcome {
    Queued,
    WakeSubmitted,
    Acknowledged,
}

/// One pending addressed message. Sender and project are inferred from the authenticated session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: AgentMessageId,
    pub project: ProjectId,
    pub sender: ProcessId,
    pub recipient: ProcessId,
    pub kind: AgentMessageKind,
    pub body: String,
    pub todo_id: Option<TodoId>,
}

/// A message paired with the state reached by the operation returning it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessageDelivery {
    pub message: AgentMessage,
    pub outcome: AgentMessageOutcome,
}

/// Compact delivery state used by broadcast responses without repeating the shared message body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessageReceipt {
    pub message_id: AgentMessageId,
    pub recipient: ProcessId,
    pub outcome: AgentMessageOutcome,
}

/// The recipients and per-message states produced by one atomic broadcast enqueue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBroadcastReceipt {
    pub deliveries: Vec<AgentMessageReceipt>,
}

/// How another live agent relates to the authenticated caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRelationship {
    #[serde(rename = "self")]
    Self_,
    Parent,
    Child,
    Sibling,
}

/// One member of the caller's live lineage group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRosterEntry {
    pub process: ProcessId,
    pub parent: Option<ProcessId>,
    pub root: ProcessId,
    pub relationship: AgentRelationship,
    pub label: String,
    pub status: ProcStatus,
}

/// A queue ceiling that refused an enqueue without dropping an existing message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxCapacityError {
    MessageTooLarge,
    RecipientQueueFull,
    ProjectQueueFull,
    GlobalQueueFull,
    GlobalByteLimit,
}
