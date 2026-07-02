//! The rmcp server handler: a stateless front over the app holding one client connection.
//!
//! The tools themselves live in [`crate::tools`], one `#[tool_router(router = …)]` block per
//! logical category; [`SoloistMcp::new`] composes their routers into the one served router via
//! [`ToolRouter`]'s `Add`. The **core** groups are always served; the **feature** groups
//! (Scratchpads, Todos, Timers, Key-Value, Prompt Templates) are gated by the user's settings —
//! they are registered only when enabled, so a disabled group's tools are neither listed nor
//! callable (Key-Value and Prompt Templates default off). Tool *names* mirror Solo for
//! interop, but the parameter schemas are clean-room — derived from the argument structs in
//! [`crate::args`]. No domain logic lives in a tool: each
//! forwards to the app, which resolves identity, scope, and the trust gate in the core, and the
//! result is returned as structured content.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::{tool_handler, ServerHandler};
use soloist_core::{McpFeatureGroup, McpToolGroups};

use crate::client::AppClient;

/// Builds one feature group's sub-router. A feature group is registered only when its setting
/// enables it, so this is invoked lazily during composition.
type FeatureGroupRouter = fn() -> ToolRouter<SoloistMcp>;

/// The Soloist MCP server: a stateless front over the app, holding one client connection.
#[derive(Clone)]
pub struct SoloistMcp {
    pub(crate) client: Arc<AppClient>,
    tool_router: ToolRouter<Self>,
}

impl SoloistMcp {
    /// Builds the handler over a client connection to the app. The core tool groups are always
    /// composed; each feature group is added only when `groups` enables it, so a disabled group is
    /// absent from `list_tools` and uncallable.
    pub fn new(client: Arc<AppClient>, groups: McpToolGroups) -> Self {
        // Core groups: always served when the MCP server runs (plan/05 §7).
        let mut tool_router = Self::identity_router()
            + Self::project_router()
            + Self::process_router()
            + Self::agent_router()
            + Self::bulk_router()
            + Self::output_router()
            + Self::services_router()
            + Self::lock_router()
            + Self::setup_router();
        // Feature groups: a registry of (group → its sub-router builder), each registered only when
        // the setting enables it. Adding a feature group is one row here plus its `McpFeatureGroup`.
        let feature_groups: [(McpFeatureGroup, FeatureGroupRouter); 5] = [
            (McpFeatureGroup::Scratchpads, Self::scratchpad_router),
            (McpFeatureGroup::Todos, Self::todo_router),
            (McpFeatureGroup::Timers, Self::timer_router),
            (McpFeatureGroup::KeyValue, Self::kv_router),
            (
                McpFeatureGroup::PromptTemplates,
                Self::prompt_template_router,
            ),
        ];
        for (group, build_router) in feature_groups {
            if groups.enabled(group) {
                tool_router += build_router();
            }
        }
        Self {
            client,
            tool_router,
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SoloistMcp {}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
