//! Authenticated agent messaging: live orchestration relationships, bounded inboxes, and completion
//! reports.
//!
//! Every handler forwards only recipient/message/todo ids and content. The core derives the sender,
//! parent relationship, and effective project from the bound session, so none can be asserted by a
//! caller. Message bounds, inbox ordering, group selection, acknowledgement, and completion rules
//! likewise remain core-owned.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{AgentMessageId, ProcessId, TodoId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{AgentMessageArg, AgentMessageBroadcastArg, AgentMessageSendArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = messaging_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "List your live orchestration family in this project: yourself, parent, children, and siblings, with process ids and statuses. Identity and relationships come from your authenticated session."
    )]
    pub(crate) async fn agent_roster(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::AgentRoster).await {
            Ok(IpcResponse::AgentRoster(agents)) => {
                structured(&serde_json::json!({ "agents": agents }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Send a bounded message to one related agent in your project. Use agent_roster for the recipient process id. The authenticated session supplies the sender and project; an optional todo_id links the message to shared work."
    )]
    pub(crate) async fn agent_message_send(
        &self,
        Parameters(AgentMessageSendArg {
            recipient,
            body,
            todo_id,
        }): Parameters<AgentMessageSendArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::AgentMessageSend {
                recipient: ProcessId::from_raw(recipient),
                body,
                todo_id: todo_id.map(TodoId::from_raw),
            })
            .await
        {
            Ok(IpcResponse::AgentMessageDelivery(delivery)) => structured(&delivery),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Broadcast one bounded message to every other live agent in your lineage-root orchestration group — parent, children, and siblings, excluding yourself. Lineage, sender, and project are derived from your authenticated session; an optional todo_id links the message to shared work."
    )]
    pub(crate) async fn agent_message_broadcast(
        &self,
        Parameters(AgentMessageBroadcastArg { body, todo_id }): Parameters<
            AgentMessageBroadcastArg,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::AgentMessageBroadcast {
                body,
                todo_id: todo_id.map(TodoId::from_raw),
            })
            .await
        {
            Ok(IpcResponse::AgentMessageBroadcast(receipt)) => structured(&receipt),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "List your pending unacknowledged inbox, oldest first. The core bounds the result to the live-run inbox limit."
    )]
    pub(crate) async fn agent_message_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::AgentMessageList).await {
            Ok(IpcResponse::AgentMessages(messages)) => {
                structured(&serde_json::json!({ "messages": messages }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Read one message addressed to you by its message_id.")]
    pub(crate) async fn agent_message_get(
        &self,
        Parameters(AgentMessageArg { message_id }): Parameters<AgentMessageArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::AgentMessageGet {
                message_id: AgentMessageId::from_raw(message_id),
            })
            .await
        {
            Ok(IpcResponse::AgentMessage(message)) => structured(&message),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Acknowledge one message addressed to you. Acknowledged messages leave the pending inbox."
    )]
    pub(crate) async fn agent_message_acknowledge(
        &self,
        Parameters(AgentMessageArg { message_id }): Parameters<AgentMessageArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::AgentMessageAcknowledge {
                message_id: AgentMessageId::from_raw(message_id),
            })
            .await
        {
            Ok(IpcResponse::AgentMessageDelivery(delivery)) => structured(&delivery),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
