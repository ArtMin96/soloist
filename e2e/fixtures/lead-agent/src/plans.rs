use std::time::Duration;

use serde::Deserialize;

pub(crate) const WORKER_TOOL_ENV: &str = "SOLOIST_E2E_WORKER_TOOL";
pub(crate) const CLOSE_SIGNAL_FILE: &str = "lead-close-signal";
pub(crate) const COORDINATION_PLAN_FILE: &str = "lead-coordination-plan";
pub(crate) const TIMER_PLAN_FILE: &str = "lead-timer-plan";
pub(crate) const MAILBOX_PLAN_FILE: &str = "lead-mailbox-plan";
pub(crate) const MAILBOX_ROLE_ARG: &str = "--soloist-e2e-mailbox-role=";
pub(crate) const SCRATCHPAD_REWRITE_FILE: &str = "lead-scratchpad-rewrite";
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(200);
pub(crate) const LINEAGE_TASK: &str = "Remain available while the lead verifies the lineage tree";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimerPlan {
    pub(crate) worker_task: String,
    pub(crate) body: String,
    pub(crate) submitted: String,
    pub(crate) max_wait_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MailboxPlan {
    pub(crate) primary: String,
    pub(crate) peer: String,
    pub(crate) todo: String,
    pub(crate) task: String,
    pub(crate) peer_task: String,
    pub(crate) broadcast: String,
    pub(crate) direct: String,
    pub(crate) direct_reply: String,
    pub(crate) lease_key: String,
    pub(crate) lease_ready: String,
    pub(crate) completion: String,
    pub(crate) proof: String,
    pub(crate) submitted: String,
    pub(crate) instructions: String,
    pub(crate) instructions_received: String,
    pub(crate) instructions_suppressed: String,
    pub(crate) task_acknowledged: String,
    pub(crate) peer_exchanged: String,
    pub(crate) completion_reported: String,
    pub(crate) primary_lease_acquired: String,
    pub(crate) peer_lease_held: String,
    pub(crate) primary_lease_released: String,
    pub(crate) peer_lease_acquired: String,
    pub(crate) peer_lease_released: String,
}

#[derive(Clone, Copy)]
pub(crate) enum MailboxRole {
    Primary,
    Peer,
}

impl MailboxRole {
    pub(crate) fn from_args() -> Option<Self> {
        std::env::args().find_map(|arg| match arg.strip_prefix(MAILBOX_ROLE_ARG) {
            Some("primary") => Some(Self::Primary),
            Some("peer") => Some(Self::Peer),
            _ => None,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoordinationPlan {
    pub(crate) scratchpad: String,
    pub(crate) body_v1: String,
    pub(crate) body_v2: String,
    pub(crate) blocker: String,
    pub(crate) blocked: String,
    pub(crate) commented: String,
    pub(crate) comment: String,
}
