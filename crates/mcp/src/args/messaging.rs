//! Parameter structs for authenticated agent-to-agent messaging tools.

use rmcp::schemars;
use serde::Deserialize;

/// Arguments for sending a message to one agent.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentMessageSendArg {
    /// The recipient process id, as returned by `agent_roster`.
    pub(crate) recipient: u64,
    /// The message body.
    pub(crate) body: String,
    /// The related todo, when this message concerns one.
    pub(crate) todo_id: Option<u64>,
}

/// Arguments for broadcasting a message to every other agent in the caller's orchestration group.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentMessageBroadcastArg {
    /// The message body.
    pub(crate) body: String,
    /// The related todo, when this message concerns one.
    pub(crate) todo_id: Option<u64>,
}

/// Arguments naming one agent message.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentMessageArg {
    /// The message id, as returned by a send, broadcast, or inbox read.
    pub(crate) message_id: u64,
}

/// Arguments for reporting completed work to a lead agent.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentCompletionArg {
    /// The todo whose work is complete.
    pub(crate) todo_id: u64,
    /// A concise account of the completed result for the lead agent.
    pub(crate) summary: String,
}
