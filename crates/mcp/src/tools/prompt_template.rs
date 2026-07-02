//! Prompt-template tools: durable reusable prompts with `{{placeholder}}` fill-ins,
//! project-scoped or global.
//!
//! Six tools cover the surface: list, read, create, update (revision-guarded), delete, and
//! export (a portable envelope that re-creates the template anywhere). The scope argument
//! defaults to the effective project; `global` shares a template across projects. Scope is
//! resolved in the core, not here. The whole group is toggleable and off by default.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::PromptScope;
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{
    PromptTemplateCreateArg, PromptTemplateListArg, PromptTemplateNameArg, PromptTemplateUpdateArg,
};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

/// The scope a tool acts in when the caller names none: the effective project, consistent
/// with every other project-scoped tool.
fn scope_or_default(scope: Option<crate::args::PromptScopeArg>) -> PromptScope {
    scope.map(PromptScope::from).unwrap_or(PromptScope::Project)
}

#[tool_router(router = prompt_template_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "List prompt templates: by default both the global ones and the current project's, or pass scope to filter. Returns each template's name, description, scope, placeholders, and revision — read one with prompt_template_read."
    )]
    pub(crate) async fn prompt_template_list(
        &self,
        Parameters(PromptTemplateListArg { scope }): Parameters<PromptTemplateListArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let scope = scope.map(PromptScope::from);
        match self
            .client
            .request(IpcRequest::PromptTemplateList { scope })
            .await
        {
            Ok(IpcResponse::PromptTemplates(templates)) => {
                structured(&serde_json::json!({ "templates": templates }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read one prompt template by name (the current project's scope unless you pass scope: global). Returns the body and the {{placeholder}} names to fill in before using it."
    )]
    pub(crate) async fn prompt_template_read(
        &self,
        Parameters(PromptTemplateNameArg { name, scope }): Parameters<PromptTemplateNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let scope = scope_or_default(scope);
        match self
            .client
            .request(IpcRequest::PromptTemplateRead { scope, name })
            .await
        {
            Ok(IpcResponse::PromptTemplate(template)) => structured(&template),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Create a prompt template. Names are unique per scope; mark fill-ins in the body with {{placeholder}}. Pass scope: global to share it across projects (default: the current project)."
    )]
    pub(crate) async fn prompt_template_create(
        &self,
        Parameters(PromptTemplateCreateArg {
            name,
            description,
            body,
            scope,
        }): Parameters<PromptTemplateCreateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let scope = scope_or_default(scope);
        match self
            .client
            .request(IpcRequest::PromptTemplateCreate {
                scope,
                name,
                description,
                body,
            })
            .await
        {
            Ok(IpcResponse::PromptTemplate(template)) => structured(&template),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Replace a prompt template's description and body, revision-guarded: pass the revision you read; a stale revision is refused — re-read and retry rather than overwriting a concurrent edit."
    )]
    pub(crate) async fn prompt_template_update(
        &self,
        Parameters(PromptTemplateUpdateArg {
            name,
            description,
            body,
            expected_revision,
            scope,
        }): Parameters<PromptTemplateUpdateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let scope = scope_or_default(scope);
        match self
            .client
            .request(IpcRequest::PromptTemplateUpdate {
                scope,
                name,
                description,
                body,
                expected_revision,
            })
            .await
        {
            Ok(IpcResponse::PromptTemplate(template)) => structured(&template),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Delete a prompt template by name and scope. Returns whether one was removed."
    )]
    pub(crate) async fn prompt_template_delete(
        &self,
        Parameters(PromptTemplateNameArg { name, scope }): Parameters<PromptTemplateNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let scope = scope_or_default(scope);
        match self
            .client
            .request(IpcRequest::PromptTemplateDelete { scope, name })
            .await
        {
            Ok(IpcResponse::PromptTemplateDeleted(deleted)) => {
                structured(&serde_json::json!({ "deleted": deleted }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Export one prompt template as a portable JSON envelope (format soloist.prompt-template/v1) you can save elsewhere or re-create via prompt_template_create."
    )]
    pub(crate) async fn prompt_template_export(
        &self,
        Parameters(PromptTemplateNameArg { name, scope }): Parameters<PromptTemplateNameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let scope = scope_or_default(scope);
        match self
            .client
            .request(IpcRequest::PromptTemplateExport { scope, name })
            .await
        {
            Ok(IpcResponse::PromptTemplateExport(exported)) => structured(&exported),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
