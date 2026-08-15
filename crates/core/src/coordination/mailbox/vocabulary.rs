use std::time::Duration;

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
/// Maximum retained transcript entries held for one project. Overflow evicts the oldest.
pub const MAX_TRANSCRIPT_ENTRIES_PER_PROJECT: usize = 512;
/// Maximum retained transcript entries held across the entire running application. Sized to one
/// full mailbox-worth, so the record can cover every message the delivery queue is able to hold.
pub const MAX_TRANSCRIPT_ENTRIES: usize = MAX_PENDING_AGENT_MESSAGES;
/// Maximum UTF-8 body bytes retained in one transcript entry; a longer body is truncated. A
/// quarter of [`MAX_AGENT_MESSAGE_BYTES`], because retaining [`MAX_TRANSCRIPT_ENTRIES`] full-size
/// bodies would exceed the whole application's idle memory budget on its own.
pub const MAX_TRANSCRIPT_BODY_BYTES: usize = 4 * 1024;
/// The longest a pending wake waits for an idle classification that may never come. A recipient
/// whose provider emits no evidence is never classified, so an idle-gated wake would wait for it
/// forever; past this the wake is delivered anyway. This is the ceiling as well as the default —
/// no caller supplies a wake deadline, so there is one value rather than a pair.
///
/// Two minutes is roughly 120 idle samples: long enough that a cold agent CLI has had ample room
/// to draw its first output or set its first title, short enough that a coordination handoff is
/// not stalled for a human-noticeable age. It is far below the fire-when-idle timer's hour-long
/// backstop because the two wait on different things — that one waits for a piece of work to
/// finish, this one only for a CLI to produce its very first signal.
pub(crate) const MAX_WAKE_WAIT: Duration = Duration::from_secs(120);

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

/// One recorded exchange, retained for display after delivery. Unlike a pending message — which
/// leaves the queue the moment its recipient acknowledges it — a record stays readable for the rest
/// of the run, including after its recipient closes. `delivery.outcome` is updated in place as the
/// message moves, so the transcript shows live delivery state rather than a stale first reading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessageRecord {
    pub delivery: AgentMessageDelivery,
    /// Wall-clock time the exchange was recorded, from [`Clock::now_unix_millis`].
    ///
    /// [`Clock::now_unix_millis`]: crate::ports::Clock::now_unix_millis
    pub at_unix_millis: u64,
    /// True when the retained body was cut at [`MAX_TRANSCRIPT_BODY_BYTES`]. The **delivered**
    /// message is never truncated — only this display copy is.
    pub truncated: bool,
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
