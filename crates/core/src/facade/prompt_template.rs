//! Session-scoped prompt-template actions (context C8 → C6): the reusable-prompt surface a remote
//! caller (MCP today) drives.
//!
//! These are thin wrappers over the unified [`Templates`](crate::coordination::Templates) aggregate
//! at [`TemplateKind::Prompt`] — the prompt-template MCP tools keep their exact names and contracts
//! while sharing one aggregate, table, and cache with the scratchpad and todo template kinds.
//! Templates are the one C6 aggregate with a **global scope** besides the project one, so every
//! method resolves the caller's chosen [`TemplateScope`] here, in the core: `Project` goes through
//! the session's effective project (like every other coordination surface), `Global` needs none. A
//! list may span both — it merges the global rows with the effective project's when one is in scope,
//! and never fails on scope alone. Every write announces [`DomainEvent::TemplateChanged`] so a
//! templates surface refreshes.

use super::scoped::ScopedFacade;
use super::template::{create_error, update_error};
use crate::coordination::{ExportedTemplate, TemplateSummary, TemplateView};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::ProjectId;
use crate::template::{TemplateKind, TemplateScope};

/// Every prompt-template action addresses this kind of the unified aggregate.
const KIND: TemplateKind = TemplateKind::Prompt;

impl ScopedFacade<'_> {
    /// The repo scope a session's template action addresses: the effective project for
    /// [`TemplateScope::Project`], nothing for [`TemplateScope::Global`].
    fn prompt_scope(&self, scope: TemplateScope) -> Result<Option<ProjectId>, CoordinationError> {
        match scope {
            TemplateScope::Global => Ok(None),
            TemplateScope::Project => Ok(Some(self.coordination_scope()?)),
        }
    }

    /// Announces a prompt-template change so a templates surface re-reads.
    fn emit_template_changed(&self) {
        self.inner
            .bus
            .publish(DomainEvent::TemplateChanged { kind: KIND });
    }

    /// Creates the template `name` in the chosen scope. A taken name is refused.
    pub fn prompt_template_create(
        &self,
        scope: TemplateScope,
        name: &str,
        description: Option<&str>,
        body: &str,
    ) -> Result<TemplateView, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        let view = self
            .inner
            .templates
            .create(KIND, project, name, description, body)
            .map_err(create_error)?;
        self.emit_template_changed();
        Ok(view)
    }

    /// Replaces the template `name`'s body, guarded by the revision the caller read. An omitted
    /// description keeps the stored one; a blank description clears it.
    pub fn prompt_template_update(
        &self,
        scope: TemplateScope,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected_revision: u64,
    ) -> Result<TemplateView, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        let view = self
            .inner
            .templates
            .update(KIND, project, name, description, body, expected_revision)
            .map_err(update_error)?;
        self.emit_template_changed();
        Ok(view)
    }

    /// The template `name` in the chosen scope.
    pub fn prompt_template_read(
        &self,
        scope: TemplateScope,
        name: &str,
    ) -> Result<TemplateView, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        self.inner
            .templates
            .read(KIND, project, name)?
            .ok_or(CoordinationError::UnknownTemplate)
    }

    /// The templates the session can address: one scope's rows when `scope` is given, else the
    /// global rows merged with the effective project's (the project half is simply absent when no
    /// project resolves — an unscoped list never fails on scope).
    pub fn prompt_template_list(
        &self,
        scope: Option<TemplateScope>,
    ) -> Result<Vec<TemplateSummary>, CoordinationError> {
        match scope {
            Some(scope) => {
                let project = self.prompt_scope(scope)?;
                Ok(self.inner.templates.list(KIND, project)?)
            }
            None => {
                let mut merged = self.inner.templates.list(KIND, None)?;
                if let Some(project) = self.inner.effective_project(self.session) {
                    merged.extend(self.inner.templates.list(KIND, Some(project))?);
                }
                // Stable by name; a name in both scopes lists its global row first (the merge order).
                merged.sort_by(|a, b| a.name.cmp(&b.name));
                Ok(merged)
            }
        }
    }

    /// Removes the template `name` from the chosen scope, returning whether one existed.
    pub fn prompt_template_delete(
        &self,
        scope: TemplateScope,
        name: &str,
    ) -> Result<bool, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        let removed = self.inner.templates.delete(KIND, project, name)?;
        if removed {
            self.emit_template_changed();
        }
        Ok(removed)
    }

    /// The template `name` as a portable export envelope.
    pub fn prompt_template_export(
        &self,
        scope: TemplateScope,
        name: &str,
    ) -> Result<ExportedTemplate, CoordinationError> {
        let project = self.prompt_scope(scope)?;
        self.inner
            .templates
            .export(KIND, project, name)?
            .ok_or(CoordinationError::UnknownTemplate)
    }
}

#[cfg(test)]
#[path = "prompt_template_tests.rs"]
mod tests;
