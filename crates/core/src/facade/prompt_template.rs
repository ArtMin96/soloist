//! Session-scoped prompt-template actions (context C8 → C6): the reusable-prompt surface a
//! remote caller (MCP today) drives.
//!
//! Templates are the one C6 aggregate with a **global scope** besides the project one, so
//! every method resolves the caller's chosen [`PromptScope`] here, in the core: `Project`
//! goes through the session's effective project (like every other coordination surface),
//! `Global` needs none. A list may span both — it merges the global rows with the effective
//! project's when one is in scope, and never fails on scope alone.

use super::scoped::ScopedFacade;
use crate::coordination::{
    ExportedPromptTemplate, PromptScope, PromptTemplateSummary, PromptTemplateView,
    PromptTemplateWriteError,
};
use crate::facade::CoordinationError;
use crate::ids::ProjectId;

impl ScopedFacade<'_> {
    /// The repo scope a session's template action addresses: the effective project for
    /// [`PromptScope::Project`], nothing for [`PromptScope::Global`].
    fn prompt_scope(&self, scope: PromptScope) -> Result<Option<ProjectId>, CoordinationError> {
        match scope {
            PromptScope::Global => Ok(None),
            PromptScope::Project => Ok(Some(self.coordination_scope()?)),
        }
    }

    /// Creates the template `name` in the chosen scope. A taken name is refused.
    pub fn prompt_template_create(
        &self,
        scope: PromptScope,
        name: &str,
        description: Option<&str>,
        body: &str,
    ) -> Result<PromptTemplateView, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        self.inner
            .prompt_templates
            .create(project, name, description, body)
            .map_err(create_error)
    }

    /// Replaces the template `name`'s body, guarded by the revision the caller read. An
    /// omitted description keeps the stored one; a blank description clears it.
    pub fn prompt_template_update(
        &self,
        scope: PromptScope,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected_revision: u64,
    ) -> Result<PromptTemplateView, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        self.inner
            .prompt_templates
            .update(project, name, description, body, expected_revision)
            .map_err(update_error)
    }

    /// The template `name` in the chosen scope.
    pub fn prompt_template_read(
        &self,
        scope: PromptScope,
        name: &str,
    ) -> Result<PromptTemplateView, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        self.inner
            .prompt_templates
            .read(project, name)?
            .ok_or(CoordinationError::UnknownPromptTemplate)
    }

    /// The templates the session can address: one scope's rows when `scope` is given, else
    /// the global rows merged with the effective project's (the project half is simply
    /// absent when no project resolves — an unscoped list never fails on scope).
    pub fn prompt_template_list(
        &self,
        scope: Option<PromptScope>,
    ) -> Result<Vec<PromptTemplateSummary>, CoordinationError> {
        match scope {
            Some(scope) => {
                let project = self.prompt_scope(scope)?;
                Ok(self.inner.prompt_templates.list(project)?)
            }
            None => {
                let mut merged = self.inner.prompt_templates.list(None)?;
                if let Some(project) = self.inner.effective_project(self.session) {
                    merged.extend(self.inner.prompt_templates.list(Some(project))?);
                }
                // Stable by name; a name in both scopes lists its global row first (the
                // merge order).
                merged.sort_by(|a, b| a.name.cmp(&b.name));
                Ok(merged)
            }
        }
    }

    /// Removes the template `name` from the chosen scope, returning whether one existed.
    pub fn prompt_template_delete(
        &self,
        scope: PromptScope,
        name: &str,
    ) -> Result<bool, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        Ok(self.inner.prompt_templates.delete(project, name)?)
    }

    /// The template `name` as a portable export envelope.
    pub fn prompt_template_export(
        &self,
        scope: PromptScope,
        name: &str,
    ) -> Result<ExportedPromptTemplate, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        self.inner
            .prompt_templates
            .export(project, name)?
            .ok_or(CoordinationError::UnknownPromptTemplate)
    }
}

/// A create's conflict means the name is taken — the actionable message for a caller that
/// did not expect the template to exist.
fn create_error(err: PromptTemplateWriteError) -> CoordinationError {
    match err {
        PromptTemplateWriteError::Conflict { .. } => CoordinationError::PromptTemplateNameTaken,
        other => update_error(other),
    }
}

fn update_error(err: PromptTemplateWriteError) -> CoordinationError {
    match err {
        PromptTemplateWriteError::Invalid(message) => {
            CoordinationError::InvalidPromptTemplate(message)
        }
        // No revision on record means no template — "re-read and retry" would mislead a
        // caller whose target was deleted.
        PromptTemplateWriteError::Conflict { actual: None, .. } => {
            CoordinationError::UnknownPromptTemplate
        }
        PromptTemplateWriteError::Conflict { expected, actual } => {
            CoordinationError::PromptTemplateRevisionConflict { expected, actual }
        }
        PromptTemplateWriteError::Store(err) => CoordinationError::Store(err),
    }
}

#[cfg(test)]
#[path = "prompt_template_tests.rs"]
mod tests;
