//! Session-authenticated agent lineage, addressed messaging, and completion reporting.

use serde::{Deserialize, Serialize};

use super::scoped::ScopedFacade;
use crate::coordination::{
    AgentBroadcastReceipt, AgentMessage, AgentMessageDelivery, AgentMessageKind,
    AgentMessageOutcome, AgentMessageReceipt, AgentRelationship, AgentRosterEntry,
    MailboxCapacityError, TodoCompletion, TodoCompletionKey, TodoCompletionNotice,
    TodoCompletionOccurrence, TodoError, MAX_AGENT_MESSAGE_BYTES,
};
use crate::ids::{AgentMessageId, ProcessId, TodoId};
use crate::ports::StoreError;
use crate::process::ProcessKind;
use crate::supervisor::SupervisorError;

/// A request that extends the legacy spawn operation with an optional addressed first task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnAgentRequest {
    pub tool: String,
    pub extra_args: Vec<String>,
    pub prompt: Option<String>,
    pub todo_id: Option<TodoId>,
    #[serde(default = "default_agent_instructions")]
    pub include_agent_instructions: bool,
}

/// The spawned worker and, when a prompt was supplied, the task message queued for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnAgentOutcome {
    pub process: ProcessId,
    pub initial_message: Option<AgentMessageDelivery>,
}

/// The durable completion record and the optional ephemeral notification queued to its live parent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionReport {
    pub completion: Option<TodoCompletion>,
    pub occurrence: Option<TodoCompletionOccurrence>,
    pub notification: CompletionNotification,
}

/// Best-effort state of the ephemeral notification associated with a durable completion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CompletionNotification {
    Enqueued { delivery: AgentMessageDelivery },
    Pending { delivery: AgentMessageDelivery },
    AlreadyQueued,
    Deferred { recipient: Option<ProcessId> },
}

/// Why an authenticated mailbox operation was refused.
#[derive(Debug, thiserror::Error)]
pub enum AgentMailboxError {
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    #[error("not bound to a live agent process")]
    NoBoundProcess,
    #[error("no such recipient in this project")]
    UnknownRecipient,
    #[error("that agent is outside your live lineage group")]
    UnrelatedRecipient,
    #[error("no pending message under that id")]
    UnknownMessage,
    #[error("the message exceeds the {MAX_AGENT_MESSAGE_BYTES}-byte cap")]
    MessageTooLarge,
    #[error("the recipient inbox is full")]
    RecipientQueueFull,
    #[error("the project mailbox is full")]
    ProjectQueueFull,
    #[error("the global mailbox is full")]
    GlobalQueueFull,
    #[error("the global mailbox byte limit is full")]
    GlobalByteLimit,
    #[error("no todo under that id")]
    UnknownTodo,
    #[error("todo is blocked by {by:?}")]
    TodoBlocked { by: Vec<TodoId> },
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

impl From<MailboxCapacityError> for AgentMailboxError {
    fn from(value: MailboxCapacityError) -> Self {
        match value {
            MailboxCapacityError::MessageTooLarge => Self::MessageTooLarge,
            MailboxCapacityError::RecipientQueueFull => Self::RecipientQueueFull,
            MailboxCapacityError::ProjectQueueFull => Self::ProjectQueueFull,
            MailboxCapacityError::GlobalQueueFull => Self::GlobalQueueFull,
            MailboxCapacityError::GlobalByteLimit => Self::GlobalByteLimit,
        }
    }
}

impl ScopedFacade<'_> {
    /// The caller and every live agent sharing its current lineage root.
    pub fn agent_roster(&self) -> Result<Vec<AgentRosterEntry>, AgentMailboxError> {
        let (project, sender) = self.mailbox_identity()?;
        let live: Vec<_> = self
            .inner
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project && view.kind == ProcessKind::Agent)
            .collect();
        let live_ids: Vec<_> = live.iter().map(|view| view.id).collect();
        let root = self.inner.lineage.root_of(sender);
        Ok(live
            .into_iter()
            .filter(|view| self.inner.lineage.root_of(view.id) == root)
            .map(|view| {
                let parent = self
                    .inner
                    .lineage
                    .parent_of(view.id)
                    .filter(|parent| live_ids.contains(parent));
                let relationship = if view.id == sender {
                    AgentRelationship::Self_
                } else if parent == Some(sender) {
                    AgentRelationship::Child
                } else if self.inner.lineage.parent_of(sender) == Some(view.id) {
                    AgentRelationship::Parent
                } else {
                    AgentRelationship::Sibling
                };
                AgentRosterEntry {
                    process: view.id,
                    parent,
                    root,
                    relationship,
                    label: view.label,
                    status: view.status,
                }
            })
            .collect())
    }

    /// Queues one direct message to a related live agent and wakes it immediately only if idle.
    pub fn agent_message_send(
        &self,
        recipient: ProcessId,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<AgentMessageDelivery, AgentMailboxError> {
        self.send_message(recipient, AgentMessageKind::Direct, body, todo_id)
    }

    /// Queues one direct message for every other member of the caller's live lineage-root group.
    pub fn agent_message_broadcast(
        &self,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<AgentBroadcastReceipt, AgentMailboxError> {
        if body.len() > MAX_AGENT_MESSAGE_BYTES {
            return Err(AgentMailboxError::MessageTooLarge);
        }
        let (project, sender) = self.mailbox_identity()?;
        let recipients: Vec<_> = self
            .agent_roster()?
            .into_iter()
            .filter(|entry| entry.process != sender)
            .map(|entry| entry.process)
            .collect();
        let mut deliveries = self.inner.mailbox.enqueue_many(
            project,
            sender,
            &recipients,
            AgentMessageKind::Direct,
            body,
            todo_id,
        )?;
        for delivery in &mut deliveries {
            let recipient = delivery.message.recipient;
            if self.inner.idle.activity(recipient) == Some(crate::agents::AgentActivity::Idle)
                && self.inner.mailbox.wake(recipient, self.inner.supervisor())
            {
                delivery.outcome = AgentMessageOutcome::WakeSubmitted;
            }
        }
        Ok(AgentBroadcastReceipt {
            deliveries: deliveries
                .into_iter()
                .map(|delivery| AgentMessageReceipt {
                    message_id: delivery.message.id,
                    recipient: delivery.message.recipient,
                    outcome: delivery.outcome,
                })
                .collect(),
        })
    }

    /// Every pending message addressed to the bound caller, in queue order.
    pub fn agent_message_list(&self) -> Result<Vec<AgentMessageDelivery>, AgentMailboxError> {
        let (_, recipient) = self.mailbox_identity()?;
        Ok(self.inner.mailbox.list(recipient))
    }

    /// One pending message addressed to the bound caller.
    pub fn agent_message_get(
        &self,
        message_id: AgentMessageId,
    ) -> Result<AgentMessageDelivery, AgentMailboxError> {
        let (_, recipient) = self.mailbox_identity()?;
        self.inner
            .mailbox
            .get(recipient, message_id)
            .ok_or(AgentMailboxError::UnknownMessage)
    }

    /// Acknowledges and removes one pending message addressed to the bound caller.
    pub fn agent_message_acknowledge(
        &self,
        message_id: AgentMessageId,
    ) -> Result<AgentMessageDelivery, AgentMailboxError> {
        let (_, recipient) = self.mailbox_identity()?;
        self.inner
            .mailbox
            .acknowledge(recipient, message_id)
            .ok_or(AgentMailboxError::UnknownMessage)
    }

    /// Reports one addressed task complete, optionally completing its associated todo atomically.
    pub fn agent_report_completion(
        &self,
        task_message_id: AgentMessageId,
        todo_id: Option<TodoId>,
        summary: String,
    ) -> Result<CompletionReport, AgentMailboxError> {
        if summary.len() > MAX_AGENT_MESSAGE_BYTES {
            return Err(AgentMailboxError::MessageTooLarge);
        }
        let (project, sender) = self.mailbox_identity()?;
        let (parent, task_todo) = self
            .inner
            .mailbox
            .task_for_completion(project, sender, task_message_id)
            .ok_or(AgentMailboxError::UnknownMessage)?;
        if todo_id != task_todo {
            return Err(AgentMailboxError::UnknownTodo);
        }
        let outcome = todo_id
            .map(|id| {
                let author = self
                    .comment_author()
                    .ok_or(AgentMailboxError::NoBoundProcess)?;
                self.inner
                    .todos
                    .report_completion(
                        project,
                        id,
                        TodoCompletionKey::new(sender, task_message_id),
                        &summary,
                        author,
                    )
                    .map_err(|error| match error {
                        TodoError::NotFound => AgentMailboxError::UnknownTodo,
                        TodoError::Blocked { by } => AgentMailboxError::TodoBlocked { by },
                        TodoError::Store(error) => AgentMailboxError::Store(error),
                        other => AgentMailboxError::Store(StoreError::Backend(other.to_string())),
                    })
            })
            .transpose()?;
        if let (Some(id), Some(outcome)) = (todo_id, &outcome) {
            if outcome.occurrence == TodoCompletionOccurrence::Recorded {
                self.inner
                    .bus
                    .publish(crate::events::DomainEvent::TodoChanged { project, id });
            }
        }
        let notification = if outcome
            .as_ref()
            .is_some_and(|outcome| outcome.notice == TodoCompletionNotice::AlreadyQueued)
        {
            CompletionNotification::AlreadyQueued
        } else if let Some(delivery) =
            todo_id.and_then(|id| self.inner.mailbox.pending_completion(parent, id))
        {
            CompletionNotification::Pending { delivery }
        } else {
            match self.send_message(parent, AgentMessageKind::Completion, summary, todo_id) {
                Ok(delivery) => {
                    if let Some(outcome) = &outcome {
                        self.inner
                            .todos
                            .mark_completion_notice_queued(project, &outcome.completion)
                            .map_err(|error| {
                                AgentMailboxError::Store(StoreError::Backend(error.to_string()))
                            })?;
                    }
                    CompletionNotification::Enqueued { delivery }
                }
                Err(_) => CompletionNotification::Deferred {
                    recipient: Some(parent),
                },
            }
        };
        Ok(CompletionReport {
            completion: outcome.as_ref().map(|outcome| outcome.completion.clone()),
            occurrence: outcome.map(|outcome| outcome.occurrence),
            notification,
        })
    }

    pub(in crate::facade) fn queue_spawned_task(
        &self,
        project: crate::ids::ProjectId,
        sender: ProcessId,
        recipient: ProcessId,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<AgentMessageDelivery, AgentMailboxError> {
        self.inner
            .mailbox
            .enqueue_reserved(
                project,
                sender,
                recipient,
                AgentMessageKind::Task,
                body,
                todo_id,
            )
            .map_err(Into::into)
    }

    fn send_message(
        &self,
        recipient: ProcessId,
        kind: AgentMessageKind,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<AgentMessageDelivery, AgentMailboxError> {
        let (project, sender) = self.mailbox_identity()?;
        let roster = self.agent_roster()?;
        if !roster.iter().any(|entry| entry.process == recipient) {
            let known_in_project = self
                .inner
                .process_view(recipient)
                .is_some_and(|view| view.project == project && view.kind == ProcessKind::Agent);
            return Err(if known_in_project {
                AgentMailboxError::UnrelatedRecipient
            } else {
                AgentMailboxError::UnknownRecipient
            });
        }
        let mut delivery = self
            .inner
            .mailbox
            .enqueue(project, sender, recipient, kind, body, todo_id)?;
        if self.inner.idle.activity(recipient) == Some(crate::agents::AgentActivity::Idle)
            && self.inner.mailbox.wake(recipient, self.inner.supervisor())
        {
            delivery.outcome = AgentMessageOutcome::WakeSubmitted;
        }
        Ok(delivery)
    }

    pub(in crate::facade) fn mailbox_identity(
        &self,
    ) -> Result<(crate::ids::ProjectId, ProcessId), AgentMailboxError> {
        let project = self
            .inner
            .effective_project(self.session)
            .ok_or(AgentMailboxError::NoProjectScope)?;
        let sender = self
            .inner
            .identity
            .origin(self.session)
            .process()
            .ok_or(AgentMailboxError::NoBoundProcess)?;
        let valid = self
            .inner
            .process_view(sender)
            .is_some_and(|view| view.project == project && view.kind == ProcessKind::Agent);
        valid
            .then_some((project, sender))
            .ok_or(AgentMailboxError::NoBoundProcess)
    }
}

const fn default_agent_instructions() -> bool {
    true
}

#[cfg(test)]
#[path = "mailbox_tests.rs"]
mod tests;
