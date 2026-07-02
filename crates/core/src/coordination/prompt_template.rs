//! The prompt-template aggregate (context C6): durable reusable prompts, global or
//! project-scoped, with `{{placeholder}}` markers filled in when a prompt is applied.
//!
//! Templates are shared mutable content like scratchpads, so writes are revision-guarded —
//! a stale update is refused, never a silent clobber. Unlike every other C6 aggregate a
//! template may live in the **global scope** (`project = None`), shared across projects;
//! names are unique per scope, and the same name may exist globally and in a project.
//! Placeholders are **derived** from the body on read, never stored, so they can never
//! disagree with the text.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::prompt_template_repo::{
    PromptTemplateRepo, PromptTemplateWriteResult, StoredPromptTemplate,
};
use crate::ids::{ProjectId, PromptTemplateId};
use crate::ports::StoreError;

/// The longest accepted template body, in bytes. A template is a prompt, not a document
/// store; together with the name and description caps this bounds the row, so a runaway
/// caller cannot grow the table without bound.
pub const MAX_PROMPT_TEMPLATE_BODY: usize = 64 * 1024;

/// The longest accepted template name, in characters — an addressing handle, not content.
pub const MAX_PROMPT_TEMPLATE_NAME: usize = 200;

/// The longest accepted template description, in characters — a one-line summary; the body
/// carries the content.
pub const MAX_PROMPT_TEMPLATE_DESCRIPTION: usize = 1_000;

/// The format discriminator of an exported template — bump on a breaking envelope change.
/// Mirrored as a literal in the `prompt_template_export` MCP tool description (a `#[tool]`
/// attribute cannot reference a const) — update both together.
pub const PROMPT_TEMPLATE_EXPORT_FORMAT: &str = "soloist.prompt-template/v1";

/// Which scope a template action addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptScope {
    Global,
    Project,
}

/// A template's full read model: the stored fields plus the placeholders derived from the
/// body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplateView {
    pub id: PromptTemplateId,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub placeholders: Vec<String>,
    pub scope: PromptScope,
    pub revision: u64,
}

impl PromptTemplateView {
    fn of(stored: StoredPromptTemplate) -> Self {
        Self {
            placeholders: placeholders(&stored.body),
            scope: scope_of(stored.project),
            id: stored.id,
            name: stored.name,
            description: stored.description,
            body: stored.body,
            revision: stored.revision,
        }
    }
}

/// A template's one-line listing: everything but the body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplateSummary {
    pub id: PromptTemplateId,
    pub name: String,
    pub description: Option<String>,
    pub placeholders: Vec<String>,
    pub scope: PromptScope,
    pub revision: u64,
}

impl PromptTemplateSummary {
    fn of(stored: &StoredPromptTemplate) -> Self {
        Self {
            id: stored.id,
            name: stored.name.clone(),
            description: stored.description.clone(),
            placeholders: placeholders(&stored.body),
            scope: scope_of(stored.project),
            revision: stored.revision,
        }
    }
}

/// The portable export envelope: enough to re-create the template anywhere via a create
/// call. Deliberately scope-free — an export is content, not placement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportedPromptTemplate {
    pub format: String,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
}

impl ExportedPromptTemplate {
    fn of(stored: &StoredPromptTemplate) -> Self {
        Self {
            format: PROMPT_TEMPLATE_EXPORT_FORMAT.to_owned(),
            name: stored.name.clone(),
            description: stored.description.clone(),
            body: stored.body.clone(),
        }
    }
}

fn scope_of(project: Option<ProjectId>) -> PromptScope {
    match project {
        None => PromptScope::Global,
        Some(_) => PromptScope::Project,
    }
}

/// Why a template write was refused.
#[derive(Debug, thiserror::Error)]
pub enum PromptTemplateWriteError {
    /// The name or body failed validation; the message says why.
    #[error("prompt template is not well-formed: {0}")]
    Invalid(String),
    /// The write expected a different revision than the one on record. `expected` is `None`
    /// for a create; `actual` is `None` when no template exists under that name.
    #[error("prompt template revision conflict (expected {expected:?}, found {actual:?})")]
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Opens a placeholder marker.
const PLACEHOLDER_OPEN: &str = "{{";
/// Closes a placeholder marker.
const PLACEHOLDER_CLOSE: &str = "}}";

/// The placeholder names a body declares with `{{name}}` markers, deduplicated, in
/// first-occurrence order — what a caller must fill in before applying the prompt.
///
/// The scan is left-to-right and the first `}}` closes a candidate. Inner text is trimmed
/// (`{{ name }}` names `name`); a candidate that trims to empty or still contains a brace
/// or newline is not a placeholder — its span is consumed as plain text, not rescanned. A
/// stray `{{` with no closing `}}` is plain text.
pub fn placeholders(body: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find(PLACEHOLDER_OPEN) {
        let candidate = &rest[start + PLACEHOLDER_OPEN.len()..];
        let Some(end) = candidate.find(PLACEHOLDER_CLOSE) else {
            break;
        };
        let name = candidate[..end].trim();
        if !name.is_empty()
            && !name.contains(['{', '}', '\n'])
            && !names.iter().any(|seen| seen == name)
        {
            names.push(name.to_owned());
        }
        rest = &candidate[end + PLACEHOLDER_CLOSE.len()..];
    }
    names
}

/// The prompt-template aggregate. Validates content and guards revisions; persistence is
/// the [`PromptTemplateRepo`] port's.
pub struct PromptTemplates {
    repo: Arc<dyn PromptTemplateRepo>,
}

impl PromptTemplates {
    pub fn new(repo: Arc<dyn PromptTemplateRepo>) -> Self {
        Self { repo }
    }

    /// Creates the template `name` in the scope. An existing name is a conflict, reported
    /// with the revision on record.
    pub fn create(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
    ) -> Result<PromptTemplateView, PromptTemplateWriteError> {
        self.write(project, name, description, body, None)
    }

    /// Replaces the template `name`'s body, guarded by the revision the caller read. An
    /// omitted description keeps the stored one; a blank description clears it. A concurrent
    /// edit landing between the keep-read and the write is caught by the revision guard.
    pub fn update(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected_revision: u64,
    ) -> Result<PromptTemplateView, PromptTemplateWriteError> {
        let kept;
        let description = match description {
            Some(text) => Some(text),
            None => {
                kept = self
                    .repo
                    .read(project, name.trim())?
                    .and_then(|stored| stored.description);
                kept.as_deref()
            }
        };
        self.write(project, name, description, body, Some(expected_revision))
    }

    fn write(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<PromptTemplateView, PromptTemplateWriteError> {
        let description = description.map(str::trim).filter(|text| !text.is_empty());
        validate(name, description, body).map_err(PromptTemplateWriteError::Invalid)?;
        match self
            .repo
            .write(project, name.trim(), description, body, expected)?
        {
            PromptTemplateWriteResult::Written(stored) => Ok(PromptTemplateView::of(*stored)),
            PromptTemplateWriteResult::Conflict { actual } => {
                Err(PromptTemplateWriteError::Conflict { expected, actual })
            }
        }
    }

    /// The template `name` in the scope, or `None`.
    pub fn read(
        &self,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<PromptTemplateView>, StoreError> {
        Ok(self
            .repo
            .read(project, name.trim())?
            .map(PromptTemplateView::of))
    }

    /// Every template in the scope as summaries, ordered by name.
    pub fn list(
        &self,
        project: Option<ProjectId>,
    ) -> Result<Vec<PromptTemplateSummary>, StoreError> {
        Ok(self
            .repo
            .list(project)?
            .iter()
            .map(PromptTemplateSummary::of)
            .collect())
    }

    /// Removes the template `name` from the scope, returning whether one was present.
    pub fn delete(&self, project: Option<ProjectId>, name: &str) -> Result<bool, StoreError> {
        self.repo.delete(project, name.trim())
    }

    /// The template `name` as a portable export envelope, or `None`.
    pub fn export(
        &self,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<ExportedPromptTemplate>, StoreError> {
        Ok(self
            .repo
            .read(project, name.trim())?
            .map(|stored| ExportedPromptTemplate::of(&stored)))
    }
}

/// Every problem with a template's content, named at once so the caller fixes it in one
/// revision.
fn validate(name: &str, description: Option<&str>, body: &str) -> Result<(), String> {
    let mut problems = Vec::new();
    if name.trim().is_empty() {
        problems.push("the name is empty".to_owned());
    } else if name.trim().chars().count() > MAX_PROMPT_TEMPLATE_NAME {
        problems.push(format!(
            "the name exceeds {MAX_PROMPT_TEMPLATE_NAME} characters"
        ));
    }
    if let Some(description) = description {
        if description.chars().count() > MAX_PROMPT_TEMPLATE_DESCRIPTION {
            problems.push(format!(
                "the description exceeds {MAX_PROMPT_TEMPLATE_DESCRIPTION} characters"
            ));
        }
    }
    if body.trim().is_empty() {
        problems.push("the body is empty".to_owned());
    }
    if body.len() > MAX_PROMPT_TEMPLATE_BODY {
        problems.push(format!(
            "the body exceeds the {} KiB cap",
            MAX_PROMPT_TEMPLATE_BODY / 1024
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

#[cfg(test)]
#[path = "prompt_template_tests.rs"]
mod tests;
