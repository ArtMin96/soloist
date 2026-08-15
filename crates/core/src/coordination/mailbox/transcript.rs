//! The bounded record of agent-to-agent exchanges retained for display.
//!
//! The delivery queue in [`state`](super::state) and this transcript are two ceilings over the same
//! aggregate with deliberately opposite policies. The queue **refuses** an enqueue past its cap,
//! because dropping a queued message would silently lose work an agent is waiting on. The
//! transcript **evicts** its oldest entry instead: it is a display structure, and refusing here
//! would make a send fail because the log was full. Recording therefore cannot fail and returns
//! nothing.

use crate::ids::{AgentMessageId, ProjectId};
use crate::sync::lock;

use super::state::{AgentMailbox, MailboxState};
use super::vocabulary::{
    AgentMessageDelivery, AgentMessageOutcome, AgentMessageRecord, MAX_TRANSCRIPT_BODY_BYTES,
    MAX_TRANSCRIPT_ENTRIES, MAX_TRANSCRIPT_ENTRIES_PER_PROJECT,
};

impl AgentMailbox {
    /// Retains one exchange in its project's transcript, truncating an oversized body and stamping
    /// it from the injected clock. Evicts the oldest entry rather than refusing, so a full
    /// transcript can never fail the send that produced it.
    pub(crate) fn record(&self, delivery: &AgentMessageDelivery) {
        let at_unix_millis = self.now_unix_millis();
        let (body, truncated) = truncate_body(&delivery.message.body);
        let mut message = delivery.message.clone();
        message.body = body;
        let record = AgentMessageRecord {
            delivery: AgentMessageDelivery {
                message,
                outcome: delivery.outcome,
            },
            at_unix_millis,
            truncated,
        };
        push(&mut lock(&self.state), record);
    }

    /// Moves an already-recorded exchange to `outcome`. A no-op when the entry has been evicted or
    /// was never recorded, so a late transition never resurrects or duplicates history.
    pub(crate) fn record_outcome(
        &self,
        project: ProjectId,
        id: AgentMessageId,
        outcome: AgentMessageOutcome,
    ) {
        advance_recorded_outcome(&mut lock(&self.state), project, id, outcome);
    }

    /// Every exchange retained for `project`, oldest first.
    pub(crate) fn transcript(&self, project: ProjectId) -> Vec<AgentMessageRecord> {
        lock(&self.state)
            .transcript
            .get(&project)
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Drops a removed project's whole transcript. The transcript's only lifecycle hook: a process
    /// closing deliberately leaves its records readable, and the two paths never share a key —
    /// inboxes are keyed per recipient, the transcript per project.
    pub(crate) fn forget_project(&self, project: ProjectId) {
        let mut state = lock(&self.state);
        if let Some(entries) = state.transcript.remove(&project) {
            state.transcript_count = state.transcript_count.saturating_sub(entries.len());
        }
    }
}

/// Updates one retained entry's outcome, reporting whether an entry was actually there to update.
/// The caller that holds a bus announces exactly what changed, so a transition against an evicted
/// entry raises no event.
pub(super) fn advance_recorded_outcome(
    state: &mut MailboxState,
    project: ProjectId,
    id: AgentMessageId,
    outcome: AgentMessageOutcome,
) -> bool {
    let Some(entries) = state.transcript.get_mut(&project) else {
        return false;
    };
    let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.delivery.message.id == id)
    else {
        return false;
    };
    entry.delivery.outcome = outcome;
    true
}

fn push(state: &mut MailboxState, record: AgentMessageRecord) {
    let project = record.delivery.message.project;
    let entries = state.transcript.entry(project).or_default();
    let replaced =
        entries.len() >= MAX_TRANSCRIPT_ENTRIES_PER_PROJECT && entries.pop_front().is_some();
    entries.push_back(record);
    if !replaced {
        state.transcript_count += 1;
    }
    while state.transcript_count > MAX_TRANSCRIPT_ENTRIES && evict_from_fullest(state) {}
}

/// Drops the oldest entry of whichever project holds the most, breaking a tie on the lowest project
/// id. The transcript is keyed per project, so "the oldest across the application" has no total
/// order to evict by; the fullest project bounds the whole without a second index, and the global
/// ceiling only binds once several projects are each near their own.
fn evict_from_fullest(state: &mut MailboxState) -> bool {
    let victim = state
        .transcript
        .iter()
        .max_by_key(|(project, entries)| (entries.len(), std::cmp::Reverse(**project)))
        .map(|(project, _)| *project);
    let Some(victim) = victim else {
        return false;
    };
    let Some(entries) = state.transcript.get_mut(&victim) else {
        return false;
    };
    if entries.pop_front().is_none() {
        return false;
    }
    if entries.is_empty() {
        state.transcript.remove(&victim);
    }
    state.transcript_count = state.transcript_count.saturating_sub(1);
    true
}

/// Cuts `body` to the retained-body cap on a UTF-8 boundary, reporting whether anything was cut.
/// Byte-slicing mid-codepoint would panic, so the cut walks back to the nearest boundary.
fn truncate_body(body: &str) -> (String, bool) {
    if body.len() <= MAX_TRANSCRIPT_BODY_BYTES {
        return (body.to_owned(), false);
    }
    let mut end = MAX_TRANSCRIPT_BODY_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    (body[..end].to_owned(), true)
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
