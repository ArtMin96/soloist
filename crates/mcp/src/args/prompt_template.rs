//! Parameter structs for the prompt-template tools.

use std::collections::BTreeMap;

use rmcp::schemars;
use serde::Deserialize;

/// The scope a prompt-template tool addresses — a closed set, mirroring the core
/// `TemplateScope` on the wire; the handler converts it. The wire name and shape are unchanged
/// (Global/Project), so the prompt-template tool contract stays byte-stable.
#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PromptScopeArg {
    Global,
    Project,
}

impl From<PromptScopeArg> for soloist_core::TemplateScope {
    fn from(scope: PromptScopeArg) -> Self {
        match scope {
            PromptScopeArg::Global => soloist_core::TemplateScope::Global,
            PromptScopeArg::Project => soloist_core::TemplateScope::Project,
        }
    }
}

/// Arguments for listing prompt templates.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct PromptTemplateListArg {
    /// Filter to one scope: "project" (the effective project's templates) or "global".
    /// Omit to list both the global templates and the current project's.
    pub(crate) scope: Option<PromptScopeArg>,
}

/// Arguments for reading, deleting, or exporting a prompt template by name.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct PromptTemplateNameArg {
    /// The template's name — the addressing handle, unique within its scope.
    pub(crate) name: String,
    /// The scope the name lives in. Omit for the current project's scope.
    pub(crate) scope: Option<PromptScopeArg>,
}

/// Arguments for rendering a prompt template into the text an agent is handed.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct PromptTemplateRenderArg {
    /// The template's name — the addressing handle, unique within its scope.
    pub(crate) name: String,
    /// The scope the name lives in. Omit for the current project's scope.
    pub(crate) scope: Option<PromptScopeArg>,
    /// A value per {{placeholder}}, keyed by the placeholder's name as prompt_template_read
    /// reports it. A placeholder you supply no value for is left in the text verbatim and named
    /// in "unfilled"; a name the body declares no placeholder for is named in "unknown". Values
    /// are substituted literally, so a value containing {{...}} lands as text.
    pub(crate) values: BTreeMap<String, String>,
}

/// Arguments for creating a prompt template.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct PromptTemplateCreateArg {
    /// The template's name — the addressing handle, unique within its scope.
    pub(crate) name: String,
    /// An optional one-line description of what the prompt is for.
    pub(crate) description: Option<String>,
    /// The prompt text. Mark fill-ins with {{placeholder}} — they are reported back as the
    /// template's placeholders, to be filled when the prompt is used.
    pub(crate) body: String,
    /// Where the template lives: "project" (the effective project) or "global" (shared
    /// across projects). Omit for the current project's scope.
    pub(crate) scope: Option<PromptScopeArg>,
}

/// Arguments for updating a prompt template.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct PromptTemplateUpdateArg {
    /// The template's name — the addressing handle, unique within its scope.
    pub(crate) name: String,
    /// The new description. Omit to keep the stored one; send an empty string to clear it.
    pub(crate) description: Option<String>,
    /// The new prompt text, replacing the old body entirely.
    pub(crate) body: String,
    /// The revision you read, guarding against a concurrent edit: a stale value is refused —
    /// re-read and retry.
    pub(crate) expected_revision: u64,
    /// The scope the name lives in. Omit for the current project's scope.
    pub(crate) scope: Option<PromptScopeArg>,
}
