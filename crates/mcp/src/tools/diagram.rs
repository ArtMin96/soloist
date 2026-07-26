//! Coordination diagram tools: the durable, project-scoped shared Mermaid-source documents agents
//! coordinate through.
//!
//! A diagram mirrors a scratchpad — addressed by its name, with revision-guarded writes — but its
//! body is a raw Mermaid **source** string rather than Markdown. Soloist stores it verbatim and never
//! renders or validates it; the desktop app renders the diagram. Writes are **revision-guarded**
//! (read, then write back the revision you read), so concurrent agents do not clobber each other.
//! Diagrams are project-scoped shared content and survive an app restart; scope is resolved in the
//! core, not here.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{
    DiagramArchiveArg, DiagramNameArg, DiagramRenameArg, DiagramTagsArg, DiagramWriteArg,
};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = diagram_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "List the diagrams in your effective project as one-line summaries (name, tags, revision, archived, and a one-line gist of the source). Diagrams are durable shared Mermaid documents that survive restarts."
    )]
    pub(crate) async fn diagram_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::DiagramList).await {
            Ok(IpcResponse::Diagrams(diagrams)) => {
                structured(&serde_json::json!({ "diagrams": diagrams }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read one diagram by its name. Returns its Mermaid source and the revision — pass that revision back to diagram_write to update it safely."
    )]
    pub(crate) async fn diagram_read(
        &self,
        Parameters(DiagramNameArg { name }): Parameters<DiagramNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::DiagramRead { name }).await {
            Ok(IpcResponse::Diagram(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Create or update a diagram from its Mermaid source — the whole diagram definition, free-form. Omit expected_revision to create; to update, read first and pass the revision you read — a mismatch means someone edited it first. Soloist stores the source verbatim and does not render or validate it."
    )]
    pub(crate) async fn diagram_write(
        &self,
        Parameters(DiagramWriteArg {
            name,
            source,
            expected_revision,
        }): Parameters<DiagramWriteArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::DiagramWrite {
            name,
            source,
            expected_revision,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Diagram(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Rename a diagram. Its durable identity is unchanged; the new name must be unused in the project."
    )]
    pub(crate) async fn diagram_rename(
        &self,
        Parameters(DiagramRenameArg { name, new_name }): Parameters<DiagramRenameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::DiagramRename { name, new_name })
            .await
        {
            Ok(IpcResponse::Diagram(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Add tags to a diagram (idempotent). Returns the updated diagram.")]
    pub(crate) async fn diagram_add_tags(
        &self,
        Parameters(DiagramTagsArg { name, tags }): Parameters<DiagramTagsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::DiagramAddTags { name, tags })
            .await
        {
            Ok(IpcResponse::Diagram(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Remove tags from a diagram. Returns the updated diagram.")]
    pub(crate) async fn diagram_remove_tags(
        &self,
        Parameters(DiagramTagsArg { name, tags }): Parameters<DiagramTagsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::DiagramRemoveTags { name, tags })
            .await
        {
            Ok(IpcResponse::Diagram(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "List the distinct tags used across your effective project's diagrams.")]
    pub(crate) async fn diagram_tags_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::DiagramTagsList).await {
            Ok(IpcResponse::DiagramTags(tags)) => structured(&serde_json::json!({ "tags": tags })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Archive a diagram (hide it from the default listing) or restore it. The document is kept — this is a flag, not a delete. Returns the updated diagram."
    )]
    pub(crate) async fn diagram_archive(
        &self,
        Parameters(DiagramArchiveArg { name, archived }): Parameters<DiagramArchiveArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::DiagramArchive { name, archived })
            .await
        {
            Ok(IpcResponse::Diagram(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Delete a diagram permanently. Returns whether one was removed under `deleted`."
    )]
    pub(crate) async fn diagram_delete(
        &self,
        Parameters(DiagramNameArg { name }): Parameters<DiagramNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::DiagramDelete { name })
            .await
        {
            Ok(IpcResponse::DiagramDeleted(deleted)) => {
                structured(&serde_json::json!({ "deleted": deleted }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
