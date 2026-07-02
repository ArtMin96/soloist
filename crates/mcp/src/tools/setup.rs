//! Setup & support tools: the usage guide, locally stored feedback, and writing the guide
//! into a project's agent-instructions file.
//!
//! `help` answers straight from the core's embedded guide — no app round-trip — so it works
//! even while Soloist itself is not running, which is exactly when an agent most needs to
//! learn what Soloist is. The other two forward to the app like every other tool.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{agent_guide, IntegrationFile};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{SetupAgentIntegrationArg, SubmitFeedbackArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = setup_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Explain how to work inside Soloist: identity and binding (SOLOIST_PROCESS_ID, bind_session_process, register_agent, whoami), project scope, the trust gate, waking up via fire-when-idle timers instead of polling, and coordination etiquette. Call this first if you are unsure how Soloist works."
    )]
    pub(crate) async fn help(&self) -> Result<CallToolResult, ErrorData> {
        structured(&serde_json::json!({ "help": agent_guide() }))
    }

    #[tool(
        description = "Record feedback about Soloist. The message is stored locally in Soloist's database for the user to review — it is never transmitted anywhere."
    )]
    pub(crate) async fn submit_solo_feedback(
        &self,
        Parameters(SubmitFeedbackArg { message }): Parameters<SubmitFeedbackArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::SubmitFeedback { message })
            .await
        {
            Ok(IpcResponse::Feedback(entry)) => structured(&entry),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Write Soloist's MCP usage guide into the current project's AGENTS.md or CLAUDE.md as a managed section — re-running replaces the section, never duplicates it. Returns the file path and whether the file was created."
    )]
    pub(crate) async fn setup_agent_integration(
        &self,
        Parameters(SetupAgentIntegrationArg { file }): Parameters<SetupAgentIntegrationArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let file = file
            .map(IntegrationFile::from)
            .unwrap_or(IntegrationFile::AgentsMd);
        match self
            .client
            .request(IpcRequest::SetupAgentIntegration { file })
            .await
        {
            Ok(IpcResponse::IntegrationWritten(write)) => structured(&write),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
