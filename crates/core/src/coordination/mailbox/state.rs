use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use crate::ids::{AgentMessageId, ProcessId, ProjectId, TodoId};
use crate::ports::Clock;
use crate::sync::lock;

use super::completion_notice::CompletionNotices;
use super::vocabulary::{
    AgentMessage, AgentMessageDelivery, AgentMessageKind, AgentMessageOutcome, AgentMessageRecord,
    MailboxCapacityError, MAX_AGENT_MESSAGE_BYTES, MAX_PENDING_AGENT_MESSAGES,
    MAX_PENDING_AGENT_MESSAGE_BYTES, MAX_PENDING_MESSAGES_PER_PROJECT,
    MAX_PENDING_MESSAGES_PER_RECIPIENT, MAX_WAKE_WAIT,
};

#[derive(Clone)]
pub(super) struct Pending {
    pub(super) message: AgentMessage,
    pub(super) outcome: AgentMessageOutcome,
}

#[derive(Default)]
pub(super) struct MailboxState {
    pub(super) inboxes: HashMap<ProcessId, VecDeque<Pending>>,
    pub(super) onboarding: HashMap<ProcessId, String>,
    project_reservations: HashMap<ProjectId, usize>,
    reserved_count: usize,
    reserved_bytes: usize,
    project_counts: HashMap<ProjectId, usize>,
    pending_count: usize,
    pending_bytes: usize,
    pub(super) pending_wakes: HashMap<ProcessId, PendingWake>,
    pub(super) wake_in_flight: HashSet<ProcessId>,
    task_receipts: VecDeque<TaskReceipt>,
    pub(super) completion_notices: CompletionNotices,
    pub(super) transcript: HashMap<ProjectId, VecDeque<AgentMessageRecord>>,
    pub(super) transcript_count: usize,
}

/// One wake owed to a recipient: how many deliveries it has refused, and the wall-clock instant
/// past which it is delivered without waiting for an idle classification at all.
pub(super) struct PendingWake {
    pub(super) attempts: u8,
    backstop_unix_millis: u64,
}

impl PendingWake {
    /// A fresh wake, with its backstop armed [`MAX_WAKE_WAIT`] out from `now`.
    pub(super) fn armed(now: u64) -> Self {
        Self {
            attempts: 0,
            backstop_unix_millis: now.saturating_add(MAX_WAKE_WAIT.as_millis() as u64),
        }
    }

    /// Whether the backstop has passed at `now`.
    fn backstop_due(&self, now: u64) -> bool {
        self.backstop_unix_millis <= now
    }
}

#[derive(Clone, Copy)]
struct TaskReceipt {
    id: AgentMessageId,
    project: ProjectId,
    sender: ProcessId,
    recipient: ProcessId,
    todo_id: Option<TodoId>,
}

const MAX_TASK_RECEIPTS: usize = MAX_PENDING_AGENT_MESSAGES;

/// The per-run mailbox. Acknowledgement removes a pending record; every queue has a hard ceiling.
pub struct AgentMailbox {
    pub(super) state: Mutex<MailboxState>,
    clock: Arc<dyn Clock>,
}

impl AgentMailbox {
    /// An empty mailbox that arms each pending wake's backstop off `clock`.
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            state: Mutex::new(MailboxState::default()),
            clock,
        }
    }

    /// The wall-clock instant a wake arming or backstop check is measured against, so every
    /// mailbox module reads time through the one injected [`Clock`] rather than the system's.
    pub(super) fn now_unix_millis(&self) -> u64 {
        self.clock.now_unix_millis()
    }

    /// The recipients whose wake has waited out [`MAX_WAKE_WAIT`], excluding any whose wake is
    /// already in flight. Read-only: the reactor decides whether each is still unclassified, and
    /// still within its retry bound, before delivering. A wake stays a candidate until it is
    /// delivered — delivery removes the pending wake outright — so a submission the recipient
    /// refuses is retried rather than lost.
    pub(super) fn backstop_candidates(&self) -> Vec<ProcessId> {
        let now = self.now_unix_millis();
        let state = lock(&self.state);
        state
            .pending_wakes
            .iter()
            .filter_map(|(recipient, pending)| {
                (pending.backstop_due(now) && !state.wake_in_flight.contains(recipient))
                    .then_some(*recipient)
            })
            .collect()
    }

    pub(crate) fn enqueue(
        &self,
        project: ProjectId,
        sender: ProcessId,
        recipient: ProcessId,
        kind: AgentMessageKind,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<AgentMessageDelivery, MailboxCapacityError> {
        if body.len() > MAX_AGENT_MESSAGE_BYTES {
            return Err(MailboxCapacityError::MessageTooLarge);
        }
        let now = self.now_unix_millis();
        let mut state = lock(&self.state);
        let recipient_len = state.inboxes.get(&recipient).map_or(0, VecDeque::len);
        if recipient_len >= MAX_PENDING_MESSAGES_PER_RECIPIENT {
            return Err(MailboxCapacityError::RecipientQueueFull);
        }
        let project_len = state.project_counts.get(&project).copied().unwrap_or(0);
        let reserved = state
            .project_reservations
            .get(&project)
            .copied()
            .unwrap_or(0);
        if project_len.saturating_add(reserved) >= MAX_PENDING_MESSAGES_PER_PROJECT {
            return Err(MailboxCapacityError::ProjectQueueFull);
        }
        if state.pending_count.saturating_add(state.reserved_count) >= MAX_PENDING_AGENT_MESSAGES {
            return Err(MailboxCapacityError::GlobalQueueFull);
        }
        if state
            .pending_bytes
            .saturating_add(state.reserved_bytes)
            .saturating_add(body.len())
            > MAX_PENDING_AGENT_MESSAGE_BYTES
        {
            return Err(MailboxCapacityError::GlobalByteLimit);
        }
        let body_bytes = body.len();
        let message = AgentMessage {
            id: AgentMessageId::next(),
            project,
            sender,
            recipient,
            kind,
            body,
            todo_id,
        };
        state
            .inboxes
            .entry(recipient)
            .or_default()
            .push_back(Pending {
                message: message.clone(),
                outcome: AgentMessageOutcome::Queued,
            });
        state.pending_count += 1;
        state.pending_bytes += body_bytes;
        *state.project_counts.entry(project).or_default() += 1;
        state
            .pending_wakes
            .entry(recipient)
            .or_insert_with(|| PendingWake::armed(now));
        Ok(AgentMessageDelivery {
            message,
            outcome: AgentMessageOutcome::Queued,
        })
    }

    pub(crate) fn enqueue_many(
        &self,
        project: ProjectId,
        sender: ProcessId,
        recipients: &[ProcessId],
        kind: AgentMessageKind,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<Vec<AgentMessageDelivery>, MailboxCapacityError> {
        if body.len() > MAX_AGENT_MESSAGE_BYTES {
            return Err(MailboxCapacityError::MessageTooLarge);
        }
        let now = self.now_unix_millis();
        let mut state = lock(&self.state);
        if recipients.iter().any(|recipient| {
            state.inboxes.get(recipient).map_or(0, VecDeque::len)
                >= MAX_PENDING_MESSAGES_PER_RECIPIENT
        }) {
            return Err(MailboxCapacityError::RecipientQueueFull);
        }
        let project_len = state.project_counts.get(&project).copied().unwrap_or(0);
        let reserved = state
            .project_reservations
            .get(&project)
            .copied()
            .unwrap_or(0);
        if project_len
            .saturating_add(reserved)
            .saturating_add(recipients.len())
            > MAX_PENDING_MESSAGES_PER_PROJECT
        {
            return Err(MailboxCapacityError::ProjectQueueFull);
        }
        if state
            .pending_count
            .saturating_add(state.reserved_count)
            .saturating_add(recipients.len())
            > MAX_PENDING_AGENT_MESSAGES
        {
            return Err(MailboxCapacityError::GlobalQueueFull);
        }
        let added_bytes = body.len().saturating_mul(recipients.len());
        if state
            .pending_bytes
            .saturating_add(state.reserved_bytes)
            .saturating_add(added_bytes)
            > MAX_PENDING_AGENT_MESSAGE_BYTES
        {
            return Err(MailboxCapacityError::GlobalByteLimit);
        }
        let deliveries = recipients
            .iter()
            .map(|recipient| {
                let message = AgentMessage {
                    id: AgentMessageId::next(),
                    project,
                    sender,
                    recipient: *recipient,
                    kind,
                    body: body.clone(),
                    todo_id,
                };
                state
                    .inboxes
                    .entry(*recipient)
                    .or_default()
                    .push_back(Pending {
                        message: message.clone(),
                        outcome: AgentMessageOutcome::Queued,
                    });
                AgentMessageDelivery {
                    message,
                    outcome: AgentMessageOutcome::Queued,
                }
            })
            .collect::<Vec<_>>();
        state.pending_count += recipients.len();
        state.pending_bytes += added_bytes;
        *state.project_counts.entry(project).or_default() += recipients.len();
        for recipient in recipients {
            state
                .pending_wakes
                .entry(*recipient)
                .or_insert_with(|| PendingWake::armed(now));
        }
        Ok(deliveries)
    }

    pub(crate) fn reserve_project_slot(
        &self,
        project: ProjectId,
        body_bytes: usize,
    ) -> Result<(), MailboxCapacityError> {
        let mut state = lock(&self.state);
        let project_len = state.project_counts.get(&project).copied().unwrap_or(0);
        let reserved = state
            .project_reservations
            .get(&project)
            .copied()
            .unwrap_or(0);
        if project_len.saturating_add(reserved) >= MAX_PENDING_MESSAGES_PER_PROJECT {
            return Err(MailboxCapacityError::ProjectQueueFull);
        }
        if state.pending_count.saturating_add(state.reserved_count) >= MAX_PENDING_AGENT_MESSAGES {
            return Err(MailboxCapacityError::GlobalQueueFull);
        }
        if state
            .pending_bytes
            .saturating_add(state.reserved_bytes)
            .saturating_add(body_bytes)
            > MAX_PENDING_AGENT_MESSAGE_BYTES
        {
            return Err(MailboxCapacityError::GlobalByteLimit);
        }
        *state.project_reservations.entry(project).or_default() += 1;
        state.reserved_count += 1;
        state.reserved_bytes += body_bytes;
        Ok(())
    }

    pub(crate) fn cancel_project_reservation(&self, project: ProjectId, body_bytes: usize) {
        let mut state = lock(&self.state);
        release_reservation(&mut state.project_reservations, project);
        state.reserved_count = state.reserved_count.saturating_sub(1);
        state.reserved_bytes = state.reserved_bytes.saturating_sub(body_bytes);
    }

    pub(crate) fn enqueue_reserved(
        &self,
        project: ProjectId,
        sender: ProcessId,
        recipient: ProcessId,
        kind: AgentMessageKind,
        body: String,
        todo_id: Option<TodoId>,
    ) -> Result<AgentMessageDelivery, MailboxCapacityError> {
        let now = self.now_unix_millis();
        let mut state = lock(&self.state);
        release_reservation(&mut state.project_reservations, project);
        state.reserved_count = state.reserved_count.saturating_sub(1);
        state.reserved_bytes = state.reserved_bytes.saturating_sub(body.len());
        if state.inboxes.get(&recipient).map_or(0, VecDeque::len)
            >= MAX_PENDING_MESSAGES_PER_RECIPIENT
        {
            return Err(MailboxCapacityError::RecipientQueueFull);
        }
        let body_bytes = body.len();
        let message = AgentMessage {
            id: AgentMessageId::next(),
            project,
            sender,
            recipient,
            kind,
            body,
            todo_id,
        };
        state
            .inboxes
            .entry(recipient)
            .or_default()
            .push_back(Pending {
                message: message.clone(),
                outcome: AgentMessageOutcome::Queued,
            });
        state.pending_count += 1;
        state.pending_bytes += body_bytes;
        *state.project_counts.entry(project).or_default() += 1;
        state
            .pending_wakes
            .entry(recipient)
            .or_insert_with(|| PendingWake::armed(now));
        Ok(AgentMessageDelivery {
            message,
            outcome: AgentMessageOutcome::Queued,
        })
    }

    pub(crate) fn list(&self, recipient: ProcessId) -> Vec<AgentMessageDelivery> {
        lock(&self.state)
            .inboxes
            .get(&recipient)
            .into_iter()
            .flatten()
            .map(|pending| AgentMessageDelivery {
                message: pending.message.clone(),
                outcome: pending.outcome,
            })
            .collect()
    }

    pub(crate) fn get(
        &self,
        recipient: ProcessId,
        id: AgentMessageId,
    ) -> Option<AgentMessageDelivery> {
        lock(&self.state)
            .inboxes
            .get(&recipient)?
            .iter()
            .find(|pending| pending.message.id == id)
            .map(|pending| AgentMessageDelivery {
                message: pending.message.clone(),
                outcome: pending.outcome,
            })
    }

    pub(crate) fn acknowledge(
        &self,
        recipient: ProcessId,
        id: AgentMessageId,
    ) -> Option<AgentMessageDelivery> {
        let now = self.now_unix_millis();
        let mut state = lock(&self.state);
        let inbox = state.inboxes.get_mut(&recipient)?;
        let index = inbox.iter().position(|pending| pending.message.id == id)?;
        let pending = inbox.remove(index)?;
        if inbox.is_empty() {
            state.inboxes.remove(&recipient);
        }
        decrement_pending(&mut state, &pending.message);
        if pending.message.kind == AgentMessageKind::Task {
            if state.task_receipts.len() == MAX_TASK_RECEIPTS {
                state.task_receipts.pop_front();
            }
            state.task_receipts.push_back(TaskReceipt {
                id: pending.message.id,
                project: pending.message.project,
                sender: pending.message.sender,
                recipient: pending.message.recipient,
                todo_id: pending.message.todo_id,
            });
        }
        if pending.outcome == AgentMessageOutcome::WakeSubmitted {
            state.wake_in_flight.remove(&recipient);
            arm_if_waiting(&mut state, recipient, now);
        }
        Some(AgentMessageDelivery {
            message: pending.message,
            outcome: AgentMessageOutcome::Acknowledged,
        })
    }

    pub(crate) fn remove_process(&self, process: ProcessId) {
        let mut state = lock(&self.state);
        if let Some(inbox) = state.inboxes.remove(&process) {
            for pending in inbox {
                decrement_pending(&mut state, &pending.message);
            }
        }
        state.onboarding.remove(&process);
        state.pending_wakes.remove(&process);
        state.wake_in_flight.remove(&process);
        // A receipt exists so its recipient can correlate the completion it still owes, so only
        // that recipient leaving retires it: a lead that exits first is exactly the case a
        // completion must survive.
        state
            .task_receipts
            .retain(|receipt| receipt.recipient != process);
        state.completion_notices.forget_process(process);
    }

    pub(crate) fn task_for_completion(
        &self,
        project: ProjectId,
        recipient: ProcessId,
        id: AgentMessageId,
    ) -> Option<(ProcessId, Option<TodoId>)> {
        let state = lock(&self.state);
        let pending = state
            .inboxes
            .get(&recipient)
            .into_iter()
            .flatten()
            .find(|pending| {
                pending.message.id == id
                    && pending.message.project == project
                    && pending.message.kind == AgentMessageKind::Task
            })
            .map(|pending| (pending.message.sender, pending.message.todo_id));
        pending.or_else(|| {
            state
                .task_receipts
                .iter()
                .find(|receipt| {
                    receipt.id == id && receipt.project == project && receipt.recipient == recipient
                })
                .map(|receipt| (receipt.sender, receipt.todo_id))
        })
    }
}

pub(super) fn arm_if_waiting(state: &mut MailboxState, recipient: ProcessId, now: u64) {
    let has_queued = state.inboxes.get(&recipient).is_some_and(|inbox| {
        inbox
            .iter()
            .any(|pending| pending.outcome == AgentMessageOutcome::Queued)
    });
    if has_queued || state.onboarding.contains_key(&recipient) {
        state
            .pending_wakes
            .insert(recipient, PendingWake::armed(now));
    }
}

fn decrement_pending(state: &mut MailboxState, message: &AgentMessage) {
    state.pending_count = state.pending_count.saturating_sub(1);
    state.pending_bytes = state.pending_bytes.saturating_sub(message.body.len());
    if let Some(count) = state.project_counts.get_mut(&message.project) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            state.project_counts.remove(&message.project);
        }
    }
}

fn release_reservation(reservations: &mut HashMap<ProjectId, usize>, project: ProjectId) {
    if let Some(count) = reservations.get_mut(&project) {
        *count = count.saturating_sub(1);
        if *count == 0 {
            reservations.remove(&project);
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
