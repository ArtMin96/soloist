//! Coordination scratchpad tools: the durable, project-scoped shared documents agents coordinate
//! through.
//!
//! A scratchpad is a **free-form Markdown note** addressed by its name — the caller writes whatever
//! structure the work needs; a project template can seed a starting shape, but nothing is enforced.
//! Writes are **revision-guarded** (read, then write back the revision you read), so concurrent
//! agents do not clobber each other. Scratchpads are project-scoped shared content and survive an
//! app restart; scope is resolved in the core, not here.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{is_link, ProjectId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{
    ScratchpadArchiveArg, ScratchpadNameArg, ScratchpadRenameArg, ScratchpadTagsArg,
    ScratchpadTransferArg, ScratchpadWriteArg,
};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = scratchpad_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "List the scratchpads in your effective project as one-line summaries (name, tags, revision, archived, and a one-line gist of the body). Scratchpads are durable shared documents that survive restarts."
    )]
    pub(crate) async fn scratchpad_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ScratchpadList).await {
            Ok(IpcResponse::Scratchpads(pads)) => {
                structured(&serde_json::json!({ "scratchpads": pads }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read one scratchpad by its name or a solo:// link to it. Returns its Markdown body, its tags, its canonical Markdown rendering (the body under its name as an H1), and the revision — pass that revision back to scratchpad_write to update it safely."
    )]
    pub(crate) async fn scratchpad_read(
        &self,
        Parameters(ScratchpadNameArg { name }): Parameters<ScratchpadNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        // A copied solo:// link routes to the scope-checked resolver; a bare value is a name.
        if is_link(&name) {
            return self.read_solo_link(name).await;
        }
        match self
            .client
            .request(IpcRequest::ScratchpadRead { name })
            .await
        {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Create or update a scratchpad from its Markdown content — the whole body, free-form. Do not repeat the name as a heading; it is the handle. Omit expected_revision to create; to update, read first and pass the revision you read — a mismatch means someone edited it first."
    )]
    pub(crate) async fn scratchpad_write(
        &self,
        Parameters(ScratchpadWriteArg {
            name,
            content,
            expected_revision,
        }): Parameters<ScratchpadWriteArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::ScratchpadWrite {
            name,
            body: content,
            expected_revision,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Rename a scratchpad. Its durable identity is unchanged; the new name must be unused in the project."
    )]
    pub(crate) async fn scratchpad_rename(
        &self,
        Parameters(ScratchpadRenameArg { name, new_name }): Parameters<ScratchpadRenameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ScratchpadRename { name, new_name })
            .await
        {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Add tags to a scratchpad (idempotent). Returns the updated scratchpad.")]
    pub(crate) async fn scratchpad_add_tags(
        &self,
        Parameters(ScratchpadTagsArg { name, tags }): Parameters<ScratchpadTagsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ScratchpadAddTags { name, tags })
            .await
        {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Remove tags from a scratchpad. Returns the updated scratchpad.")]
    pub(crate) async fn scratchpad_remove_tags(
        &self,
        Parameters(ScratchpadTagsArg { name, tags }): Parameters<ScratchpadTagsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ScratchpadRemoveTags { name, tags })
            .await
        {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "List the distinct tags used across your effective project's scratchpads."
    )]
    pub(crate) async fn scratchpad_tags_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ScratchpadTagsList).await {
            Ok(IpcResponse::ScratchpadTags(tags)) => {
                structured(&serde_json::json!({ "tags": tags }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Archive a scratchpad (hide it from the default listing) or restore it. The document is kept — this is a flag, not a delete. Returns the updated scratchpad."
    )]
    pub(crate) async fn scratchpad_archive(
        &self,
        Parameters(ScratchpadArchiveArg { name, archived }): Parameters<ScratchpadArchiveArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ScratchpadArchive { name, archived })
            .await
        {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Delete a scratchpad permanently. Returns whether one was removed under `deleted`."
    )]
    pub(crate) async fn scratchpad_delete(
        &self,
        Parameters(ScratchpadNameArg { name }): Parameters<ScratchpadNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ScratchpadDelete { name })
            .await
        {
            Ok(IpcResponse::ScratchpadDeleted(deleted)) => {
                structured(&serde_json::json!({ "deleted": deleted }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Move a scratchpad to another project, keeping its document, revision, tags, and durable identity. You must be authenticated to the destination — a process you run in belongs to it; a name already used there is refused. Note: an MCP session is scoped to a single project, so a genuine cross-project move over MCP is refused; the desktop app performs cross-project transfers."
    )]
    pub(crate) async fn scratchpad_transfer(
        &self,
        Parameters(ScratchpadTransferArg { name, to_project }): Parameters<ScratchpadTransferArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ScratchpadTransfer {
                name,
                to_project: ProjectId::from_raw(to_project),
            })
            .await
        {
            Ok(IpcResponse::Scratchpad(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
