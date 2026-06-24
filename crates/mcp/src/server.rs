//! The rmcp server handler: a stateless front over the app holding one client connection.
//!
//! The tools themselves live in [`crate::tools`], one `#[tool_router(router = …)]` block per
//! logical category; [`SoloistMcp::new`] composes their routers into the one served router via
//! [`ToolRouter`]'s `Add`. Tool *names* mirror Solo for interop, but the parameter schemas are
//! clean-room — derived from the argument structs in [`crate::args`]. No domain logic lives in
//! a tool: each forwards to the app, which resolves identity, scope, and the trust gate in the
//! core, and the result is returned as structured content.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::{tool_handler, ServerHandler};

use crate::client::AppClient;

/// The Soloist MCP server: a stateless front over the app, holding one client connection.
#[derive(Clone)]
pub struct SoloistMcp {
    pub(crate) client: Arc<AppClient>,
    tool_router: ToolRouter<Self>,
}

impl SoloistMcp {
    /// Builds the handler over a client connection to the app, composing every tool category's
    /// router into the one served router.
    pub fn new(client: Arc<AppClient>) -> Self {
        Self {
            client,
            tool_router: Self::identity_router()
                + Self::project_router()
                + Self::process_router()
                + Self::agent_router()
                + Self::bulk_router()
                + Self::output_router()
                + Self::services_router()
                + Self::lock_router()
                + Self::timer_router()
                + Self::scratchpad_router()
                + Self::todo_router()
                + Self::kv_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SoloistMcp {}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
