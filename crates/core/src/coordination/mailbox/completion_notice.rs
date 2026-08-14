//! The bounded per-run record of which reported tasks already put a completion notice in the
//! mailbox. A report is identified by the `(reporter, task message)` pair it always carries — the
//! todo it names is optional correlation — so repeating one report never enqueues a second notice,
//! whether or not the task carried a todo and whether or not the lead has read the first.

use std::collections::VecDeque;

use crate::ids::{AgentMessageId, ProcessId};
use crate::sync::lock;

use super::state::AgentMailbox;
use super::vocabulary::{AgentMessage, AgentMessageDelivery, MAX_PENDING_AGENT_MESSAGES};

/// Maximum notices remembered after their message has left the recipient's inbox.
const MAX_COMPLETION_NOTICES: usize = MAX_PENDING_AGENT_MESSAGES;

/// Where the notice a reporter already sent for one task now stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CompletionNoticeState {
    /// Still waiting, unread, in its recipient's inbox.
    Pending(AgentMessageDelivery),
    /// Acknowledged by its recipient, so only the record of it remains.
    Acknowledged,
}

/// One notice a reporter put in the mailbox, tied to the task it resolves.
#[derive(Clone, Copy)]
struct NoticeReceipt {
    reporter: ProcessId,
    task_message: AgentMessageId,
    notice: AgentMessageId,
    recipient: ProcessId,
}

/// The bounded ring of notices sent this run; the oldest is dropped at the ceiling.
#[derive(Default)]
pub(super) struct CompletionNotices {
    receipts: VecDeque<NoticeReceipt>,
}

impl CompletionNotices {
    fn record(&mut self, receipt: NoticeReceipt) {
        if self.receipts.len() == MAX_COMPLETION_NOTICES {
            self.receipts.pop_front();
        }
        self.receipts.push_back(receipt);
    }

    fn find(&self, reporter: ProcessId, task_message: AgentMessageId) -> Option<NoticeReceipt> {
        self.receipts
            .iter()
            .find(|receipt| receipt.reporter == reporter && receipt.task_message == task_message)
            .copied()
    }

    pub(super) fn forget_process(&mut self, process: ProcessId) {
        self.receipts
            .retain(|receipt| receipt.reporter != process && receipt.recipient != process);
    }
}

impl AgentMailbox {
    /// Records that `reporter` queued `notice` to resolve the task `task_message`.
    pub(crate) fn record_completion_notice(
        &self,
        reporter: ProcessId,
        task_message: AgentMessageId,
        notice: &AgentMessage,
    ) {
        lock(&self.state).completion_notices.record(NoticeReceipt {
            reporter,
            task_message,
            notice: notice.id,
            recipient: notice.recipient,
        });
    }

    /// Where the notice `reporter` already sent for `task_message` stands, or `None` when this run
    /// holds no record of one and the report still owes its lead a notice.
    pub(crate) fn completion_notice(
        &self,
        reporter: ProcessId,
        task_message: AgentMessageId,
    ) -> Option<CompletionNoticeState> {
        let state = lock(&self.state);
        let receipt = state.completion_notices.find(reporter, task_message)?;
        let pending = state
            .inboxes
            .get(&receipt.recipient)
            .into_iter()
            .flatten()
            .find(|pending| pending.message.id == receipt.notice)
            .map(|pending| AgentMessageDelivery {
                message: pending.message.clone(),
                outcome: pending.outcome,
            });
        Some(pending.map_or(
            CompletionNoticeState::Acknowledged,
            CompletionNoticeState::Pending,
        ))
    }
}
