//! Shared value types for templates: the closed [`TemplateKind`] and [`TemplateScope`]
//! discriminators.
//!
//! These are value vocabulary — like [`process`](crate::process) — owned by no context. The
//! coordination aggregate that stores templates and the event bus that announces their changes
//! both depend on this module, so it must depend on nothing itself; that is what keeps the graph
//! acyclic (a [`crate::events::DomainEvent`] carries a `TemplateKind`).

use serde::{Deserialize, Serialize};

/// Which kind of document a template seeds. A closed set: a template belongs to exactly one kind,
/// and every match over it is exhaustive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    /// A reusable prompt with `{{placeholder}}` fill-ins, applied to an agent by name.
    Prompt,
    /// A starting shape for a new scratchpad's Markdown body.
    Scratchpad,
    /// A starting shape for a new todo's Markdown body.
    Todo,
}

impl TemplateKind {
    /// Every kind, in display order — the single source a UI or a persisted-shape check iterates.
    pub const ALL: [TemplateKind; 3] = [
        TemplateKind::Prompt,
        TemplateKind::Scratchpad,
        TemplateKind::Todo,
    ];

    /// The persisted discriminator stored in the `kind` column, matching the serde `snake_case`
    /// used on the wire so the durable form and the JSON form never disagree.
    pub fn as_str(self) -> &'static str {
        match self {
            TemplateKind::Prompt => "prompt",
            TemplateKind::Scratchpad => "scratchpad",
            TemplateKind::Todo => "todo",
        }
    }

    /// Parses a persisted [`as_str`](Self::as_str) discriminator back into a kind, or `None` for an
    /// unrecognised value — so a store adapter maps a corrupt row to an error rather than panicking.
    pub fn from_db(value: &str) -> Option<Self> {
        TemplateKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
    }

    /// The portable export envelope's `format` tag for this kind — the version string a saved
    /// export carries so a re-create can recognise it.
    pub fn export_format(self) -> &'static str {
        match self {
            // Repeated as a literal in the `prompt_template_export` MCP tool description, which a
            // `#[tool(description = …)]` string cannot reference — update both together.
            TemplateKind::Prompt => "soloist.prompt-template/v1",
            TemplateKind::Scratchpad => "soloist.scratchpad-template/v1",
            TemplateKind::Todo => "soloist.todo-template/v1",
        }
    }
}

/// Which scope a template action addresses: the global library shared across projects, or one
/// project's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateScope {
    Global,
    Project,
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
