//! Parameter structs for the setup/support tools: feedback and the integration-guide write.

use rmcp::schemars;
use serde::Deserialize;

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
