use crate::ids::{ProcessId, ProjectId};
use crate::sync::lock;

use super::state::{AgentMailbox, PendingWake};
use super::vocabulary::AgentMessageOutcome;

/// The authenticated context rendered into a spawned agent's orchestration instructions.
pub struct OrchestrationGuide<'a> {
    pub process: ProcessId,
    pub project: ProjectId,
    pub label: &'a str,
}

/// Renders the single core-owned orchestration preamble used for spawned agents.
pub fn orchestration_guide(context: OrchestrationGuide<'_>) -> String {
    format!(
        "[Soloist orchestration context]\nAuthenticated identity: {} ({}) in project {}. Use whoami and agent_roster for identity and lineage; agent_message_list, agent_message_get, and agent_message_acknowledge for addressed work; agent_message_send and agent_message_broadcast to coordinate; agent_report_completion to report a todo result.",
        context.label, context.process, context.project
    )
}

impl AgentMailbox {
    pub(crate) fn queue_onboarding(&self, process: ProcessId, guide: String) {
        let now = self.now_unix_millis();
        let mut state = lock(&self.state);
        state.onboarding.insert(process, guide);
        state.pending_wakes.insert(process, PendingWake::armed(now));
    }

    pub(super) fn claim_wake_envelope(
        &self,
        recipient: ProcessId,
    ) -> Option<(String, Vec<crate::ids::AgentMessageId>)> {
        let mut state = lock(&self.state);
        if state.wake_in_flight.contains(&recipient) {
            return None;
        }
        let onboarding = state.onboarding.get(&recipient);
        let queued: Vec<_> = state
            .inboxes
            .get(&recipient)
            .into_iter()
            .flatten()
            .filter(|pending| pending.outcome == AgentMessageOutcome::Queued)
            .map(|pending| pending.message.id)
            .collect();
        if onboarding.is_none() && queued.is_empty() {
            return None;
        }
        let ids = queued
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let waiting = (!queued.is_empty()).then(|| {
            format!("Addressed message(s) {ids} are waiting. Call agent_message_get, then agent_message_acknowledge after accepting each.")
        });
        let envelope = onboarding
            .into_iter()
            .chain(waiting.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        state.wake_in_flight.insert(recipient);
        Some((envelope, queued))
    }

    /// Moves the claimed messages to [`WakeSubmitted`](AgentMessageOutcome::WakeSubmitted) and
    /// reports which of them a retained record was updated for. Every wake path runs through here
    /// — the synchronous one a send takes when its recipient is already idle, and the reactor's
    /// idle-driven and backstop deliveries — so the record stays truthful without the mailbox
    /// holding a bus. A send records its own exchange only after its wake attempt, so nothing is
    /// reported back to it; the reactor, which wakes long after the record exists, gets the
    /// entries to announce.
    pub(crate) fn mark_wake_submitted(
        &self,
        recipient: ProcessId,
        claimed: &[crate::ids::AgentMessageId],
    ) -> Vec<(ProjectId, crate::ids::AgentMessageId)> {
        let mut state = lock(&self.state);
        state.onboarding.remove(&recipient);
        state.pending_wakes.remove(&recipient);
        let mut woken = Vec::new();
        if let Some(inbox) = state.inboxes.get_mut(&recipient) {
            for pending in inbox {
                if pending.outcome == AgentMessageOutcome::Queued
                    && claimed.contains(&pending.message.id)
                {
                    pending.outcome = AgentMessageOutcome::WakeSubmitted;
                    woken.push((pending.message.project, pending.message.id));
                }
            }
        }
        woken.retain(|(project, id)| {
            super::transcript::advance_recorded_outcome(
                &mut state,
                *project,
                *id,
                AgentMessageOutcome::WakeSubmitted,
            )
        });
        woken
    }

    pub(super) fn release_wake_claim(&self, recipient: ProcessId) {
        lock(&self.state).wake_in_flight.remove(&recipient);
    }

    pub(super) fn observe_non_idle(&self, recipient: ProcessId) {
        let now = self.now_unix_millis();
        let mut state = lock(&self.state);
        if state.wake_in_flight.remove(&recipient) {
            super::state::arm_if_waiting(&mut state, recipient, now);
        }
    }
}
