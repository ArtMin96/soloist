//! Parameter structs for the setup/support tools: the guide lookup, feedback, and the
//! integration-guide write.

use rmcp::schemars;
use serde::Deserialize;

/// Arguments for the `help` tool. A topic is optional: omit it for the compact capability
/// overview, or name a topic (or one of its aliases) for detail on one area.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub(crate) struct HelpArg {
    /// The topic to explain — a key like `timers` or an alias like `ports`, `status`, or
    /// `how do I`. Omit for the capability overview and the list of topics.
    pub(crate) topic: Option<String>,
}

/// Arguments for submitting feedback.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SubmitFeedbackArg {
    /// The feedback text. Stored locally in Soloist's own database for the user to review —
    /// it is never transmitted anywhere.
    pub(crate) message: String,
}

/// The agent-instructions file `setup_agent_integration` writes — a closed set, mirroring the
/// core `IntegrationFile` on the wire; the handler converts it.
#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntegrationFileArg {
    AgentsMd,
    ClaudeMd,
}

impl From<IntegrationFileArg> for soloist_core::IntegrationFile {
    fn from(file: IntegrationFileArg) -> Self {
        match file {
            IntegrationFileArg::AgentsMd => soloist_core::IntegrationFile::AgentsMd,
            IntegrationFileArg::ClaudeMd => soloist_core::IntegrationFile::ClaudeMd,
        }
    }
}

/// Arguments for writing the integration guide into the project.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SetupAgentIntegrationArg {
    /// Which file in the project root to write the guide into. Omit for AGENTS.md.
    pub(crate) file: Option<IntegrationFileArg>,
}
