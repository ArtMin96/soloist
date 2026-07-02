//! Project tools: listing projects and reading a project's status.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::ProjectId;
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::ProjectArg;
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = project_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(description = "List every project Soloist has open.")]
    pub(crate) async fn list_projects(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListProjects).await {
            // Wrapped in an object: the MCP spec requires `structuredContent` to be a JSON
            // object, so a list reply must never be a bare array (clients refuse it).
            Ok(IpcResponse::Projects(projects)) => {
                structured(&serde_json::json!({ "projects": projects }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Get a project and its processes. Omit `project` to use the session's effective project."
    )]
    pub(crate) async fn get_project_status(
        &self,
        Parameters(ProjectArg { project }): Parameters<ProjectArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GetProjectStatus {
            project: project.map(ProjectId::from_raw),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::ProjectStatus(status)) => structured(&status),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
