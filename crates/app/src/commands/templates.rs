//! The Templates-manager commands (the Settings Templates tab).
//!
//! Thin wrappers that route straight to the one [`Facade`] — no logic here. Each is a **local**,
//! trusted read/write over the global template library the manager edits; kind grouping, name
//! uniqueness, the revision guard, and clearing a deleted default all live in the core, so these
//! only marshal arguments and map the typed error to a string the UI renders. Every write emits the
//! `TemplateChanged` domain event the panel live-refreshes on.

use std::sync::Arc;

use soloist_core::{Facade, TemplateKind, TemplateSummary, TemplateView};
use tauri::State;

/// Every global template of `kind`, ordered by name — the manager's list for one kind.
#[tauri::command]
pub async fn templates(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
) -> Result<Vec<TemplateSummary>, String> {
    facade
        .blocking(move |f| f.templates(kind))
        .await
        .map_err(|err| err.to_string())
}

/// The full global template `name` of `kind` — its body, description, and revision — for the manager
/// to open and edit.
#[tauri::command]
pub async fn template_read(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    name: String,
) -> Result<TemplateView, String> {
    facade
        .blocking(move |f| f.template_read(kind, &name))
        .await
        .map_err(|err| err.to_string())
}

/// Creates the global template `name` of `kind` with `body` and an optional one-line description. A
/// taken name or a blank name/body returns the core's rejection as an error string.
#[tauri::command]
pub async fn template_create(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    name: String,
    description: Option<String>,
    body: String,
) -> Result<TemplateView, String> {
    facade
        .blocking(move |f| f.template_create(kind, &name, description.as_deref(), &body))
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the global template `name`'s body and description, revision-guarded by
/// `expected_revision`. A stale revision returns the conflict as an error string for the panel.
#[tauri::command]
pub async fn template_update(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    name: String,
    description: Option<String>,
    body: String,
    expected_revision: u64,
) -> Result<TemplateView, String> {
    facade
        .blocking(move |f| {
            f.template_update(
                kind,
                &name,
                description.as_deref(),
                &body,
                expected_revision,
            )
        })
        .await
        .map_err(|err| err.to_string())
}

/// Removes the global template `name` of `kind`, returning whether one existed. The core clears a
/// default selection that pointed at it.
#[tauri::command]
pub async fn template_delete(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    name: String,
) -> Result<bool, String> {
    facade
        .blocking(move |f| f.template_delete(kind, &name))
        .await
        .map_err(|err| err.to_string())
}
