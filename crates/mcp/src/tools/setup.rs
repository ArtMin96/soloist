//! Setup & support tools: the usage guide, locally stored feedback, and writing the guide
//! into a project's agent-instructions file.
//!
//! `help` answers straight from the core's embedded guide — no app round-trip — so it works
//! even while Soloist itself is not running, which is exactly when an agent most needs to
//! learn what Soloist is. The other two forward to the app like every other tool.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{help_overview, help_topic, IntegrationFile};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{HelpArg, SetupAgentIntegrationArg, SubmitFeedbackArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = setup_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Learn how to work inside Soloist. Call with no topic for a compact capability overview and the list of topics; call with a topic (e.g. timers, scope, trust) or an alias (ports, services, status, \"how do I\", yaml) for detail on one area. Answered from the server itself, so it works even while Soloist is not running. Call this first if you are unsure how Soloist works."
    )]
    pub(crate) async fn help(
        &self,
        Parameters(HelpArg { topic }): Parameters<HelpArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match topic.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            // No topic: the capability overview and the topic menu.
            None => structured(&serde_json::json!({ "help": help_overview() })),
            // A known topic: just that section. An unknown one falls back to the overview, with the
            // query echoed so the agent sees why, rather than erroring on a guess.
            Some(topic) => match help_topic(topic) {
                Some(section) => {
                    structured(&serde_json::json!({ "topic": topic, "help": section }))
                }
                None => structured(&serde_json::json!({
                    "unknown_topic": topic,
                    "help": help_overview(),
                })),
            },
        }
    }

    #[tool(
        description = "List the currently enabled Soloist MCP tools, grouped by category, as names with one-line summaries and no input schemas — a compact map of the whole surface. Use it to see everything available without the weight of a full tool listing."
    )]
    pub(crate) async fn mcp_tools_summary(&self) -> Result<CallToolResult, ErrorData> {
        structured(&self.tools_summary())
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
