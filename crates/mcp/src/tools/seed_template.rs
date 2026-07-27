//! The one seed-template read behind the per-kind peek tools.
//!
//! Scratchpads and todos each expose their own tool so the peek is gated by the same feature group
//! as the create it describes — turn Scratchpads off and the scratchpad peek goes with it. Both
//! tools are the same request, so it lives here once rather than in each category file.

use rmcp::model::{CallToolResult, ErrorData};
use soloist_core::TemplateKind;
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

impl SoloistMcp {
    /// The template a new document of `kind` would be seeded from, as `{"template": …}` — `null`
    /// when the user has selected no default for that kind.
    pub(crate) async fn seed_template(
        &self,
        kind: TemplateKind,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::SeedTemplateRead { kind })
            .await
        {
            Ok(IpcResponse::SeedTemplate(template)) => {
                structured(&serde_json::json!({ "template": template }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
