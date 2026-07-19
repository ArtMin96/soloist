//! The Templates-manager commands (the Settings Templates tab).
//!
//! Thin wrappers that route straight to the one [`Facade`] — no logic here. Each is a **local**,
//! trusted read/write against the scope the caller names: `project = None` is the global library,
//! `Some` is that project's. The local user is not scope-limited, so the manager reaches both
//! libraries an MCP caller reaches. Kind grouping, name uniqueness, the revision guard, and clearing
//! a deleted default all live in the core, so these only marshal arguments and map the typed error
//! to a string the UI renders. Every write emits the `TemplateChanged` domain event — carrying the
//! same scope — that the panel live-refreshes on.

use std::collections::BTreeMap;
use std::sync::Arc;

use soloist_core::{
    Facade, MissingPolicy, ProjectId, RenderRequest, RenderedPrompt, TemplateKind, TemplateSummary,
    TemplateView,
};
use tauri::State;

/// Every template of `kind` in `project`'s scope, ordered by name — the manager's list for one kind
/// and scope.
#[tauri::command]
pub async fn templates(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    project: Option<ProjectId>,
) -> Result<Vec<TemplateSummary>, String> {
    facade
        .blocking(move |f| f.templates(kind, project))
        .await
        .map_err(|err| err.to_string())
}

/// The full template `name` of `kind` in `project`'s scope — its body, description, and revision —
/// for the manager to open and edit.
#[tauri::command]
pub async fn template_read(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    project: Option<ProjectId>,
    name: String,
) -> Result<TemplateView, String> {
    facade
        .blocking(move |f| f.template_read(kind, project, &name))
        .await
        .map_err(|err| err.to_string())
}

/// Creates the template `name` of `kind` in `project`'s scope with `body` and an optional one-line
/// description. A taken name or a blank name/body returns the core's rejection as an error string.
#[tauri::command]
pub async fn template_create(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    project: Option<ProjectId>,
    name: String,
    description: Option<String>,
    body: String,
) -> Result<TemplateView, String> {
    facade
        .blocking(move |f| f.template_create(kind, project, &name, description.as_deref(), &body))
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the template `name`'s body and description in `project`'s scope, revision-guarded by
/// `expected_revision`. A stale revision returns the conflict as an error string for the panel.
#[tauri::command]
pub async fn template_update(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    project: Option<ProjectId>,
    name: String,
    description: Option<String>,
    body: String,
    expected_revision: u64,
) -> Result<TemplateView, String> {
    facade
        .blocking(move |f| {
            f.template_update(
                kind,
                project,
                &name,
                description.as_deref(),
                &body,
                expected_revision,
            )
        })
        .await
        .map_err(|err| err.to_string())
}

/// The render the manager's preview issues.
///
/// The preview renders under [`MissingPolicy::LeaveVerbatim`] because its job is to show the gap: a
/// placeholder with no value stays literal in the text and comes back named in `unfilled`, so the
/// user sees both the missing token in the output and the list of what to fill. Refusing the render
/// instead would leave the pane blank at exactly the moment it has something to say.
fn preview_request(name: String, values: BTreeMap<String, String>) -> RenderRequest {
    RenderRequest {
        name,
        values,
        policy: MissingPolicy::LeaveVerbatim,
    }
}

/// The prompt template `name` in `project`'s scope, substituted with `values` — what the manager's
/// preview shows.
#[tauri::command]
pub async fn template_render(
    facade: State<'_, Arc<Facade>>,
    project: Option<ProjectId>,
    name: String,
    values: BTreeMap<String, String>,
) -> Result<RenderedPrompt, String> {
    let request = preview_request(name, values);
    facade
        .blocking(move |f| f.template_render(project, &request))
        .await
        .map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "templates_tests.rs"]
mod tests;

/// Removes the template `name` of `kind` from `project`'s scope, returning whether one existed. The
/// core clears a default selection that pointed at it.
#[tauri::command]
pub async fn template_delete(
    facade: State<'_, Arc<Facade>>,
    kind: TemplateKind,
    project: Option<ProjectId>,
    name: String,
) -> Result<bool, String> {
    facade
        .blocking(move |f| f.template_delete(kind, project, &name))
        .await
        .map_err(|err| err.to_string())
}
