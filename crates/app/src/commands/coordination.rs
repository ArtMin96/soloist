//! The coordination-panel commands (the scratchpad panel and the to-do board).
//!
//! Thin wrappers that route straight to the one [`Facade`] — no logic here. Each is a **local**
//! read/write like [`orchestration_snapshot`](super::orchestration_snapshot): the trusted local UI
//! hands the `project` it already has access to (the façade's project-scoped `*_in` methods), so
//! these are registered only for the Tauri surface and never expose a caller-chosen project to
//! MCP/HTTP. Writes emit the coordination domain events the panels live-refresh on. The board never
//! locks a todo — a lock is a signal an agent owns, surfaced read-only in the snapshot.

use std::sync::Arc;

use soloist_core::{
    Facade, ProjectId, ScratchpadId, ScratchpadLink, ScratchpadView, TodoDoc, TodoId, TodoView,
};
use tauri::State;

/// The full scratchpad `name` in `project` — its Markdown body, rendering, and revision — for the
/// panel to open and edit.
#[tauri::command]
pub async fn scratchpad_read(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    name: String,
) -> Result<ScratchpadView, String> {
    facade
        .blocking(move |f| f.scratchpad_read_in(project, &name))
        .await
        .map_err(|err| err.to_string())
}

/// Saves the scratchpad `name` in `project` with the Markdown `body`, revision-guarded by
/// `expected_revision` (omit to create). A stale revision returns the conflict as an error string
/// for the panel to surface.
#[tauri::command]
pub async fn scratchpad_write(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    name: String,
    body: String,
    expected_revision: Option<u64>,
) -> Result<ScratchpadView, String> {
    facade
        .blocking(move |f| f.scratchpad_write_in(project, &name, body, expected_revision))
        .await
        .map_err(|err| err.to_string())
}

/// Archives or restores the scratchpad `name` in `project` — a listing flag, not a delete. Routes to
/// the one core archive behaviour the MCP surface also drives; the panel re-reads on the emitted
/// `ScratchpadChanged`.
#[tauri::command]
pub async fn scratchpad_archive(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    name: String,
    archived: bool,
) -> Result<ScratchpadView, String> {
    facade
        .blocking(move |f| f.scratchpad_archive_in(project, &name, archived))
        .await
        .map_err(|err| err.to_string())
}

/// Renames the scratchpad `from` to `to` in `project`, keeping its durable id and revision. Routes
/// to the one core rename the MCP `scratchpad_rename` tool also drives; a name already in use comes
/// back as an error string for the header's rename field to surface.
#[tauri::command]
pub async fn scratchpad_rename(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    from: String,
    to: String,
) -> Result<ScratchpadView, String> {
    facade
        .blocking(move |f| f.scratchpad_rename_in(project, &from, &to))
        .await
        .map_err(|err| err.to_string())
}

/// Creates a todo from the disciplined `doc` in `project`, associated with `scratchpad` when the
/// board's picker named one.
#[tauri::command]
pub async fn todo_create(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    doc: TodoDoc,
    scratchpad: Option<ScratchpadId>,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| f.todo_create_in(project, doc, scratchpad))
        .await
        .map_err(|err| err.to_string())
}

/// Replaces todo `id`'s document in `project`, revision-guarded by `expected_revision`, and sets its
/// association to `scratchpad`. The board's picker is always on screen, so this surface states the
/// association on every write — hence a plain optional id rather than the wire's three-state link.
#[tauri::command]
pub async fn todo_update(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    id: TodoId,
    doc: TodoDoc,
    scratchpad: Option<ScratchpadId>,
    expected_revision: u64,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| {
            f.todo_update_in(
                project,
                id,
                doc,
                ScratchpadLink::stated(scratchpad),
                expected_revision,
            )
        })
        .await
        .map_err(|err| err.to_string())
}

/// Marks todo `id` done in `project` — refused (as an error string) while it has unmet blockers.
#[tauri::command]
pub async fn todo_complete(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    id: TodoId,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| f.todo_complete_in(project, id))
        .await
        .map_err(|err| err.to_string())
}

/// Replaces todo `id`'s blockers in `project`.
#[tauri::command]
pub async fn todo_set_blockers(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    id: TodoId,
    blockers: Vec<TodoId>,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| f.todo_set_blockers_in(project, id, blockers))
        .await
        .map_err(|err| err.to_string())
}

/// Adds one blocker to todo `id` in `project`.
#[tauri::command]
pub async fn todo_add_blocker(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    id: TodoId,
    blocker: TodoId,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| f.todo_add_blocker_in(project, id, blocker))
        .await
        .map_err(|err| err.to_string())
}

/// Removes one blocker from todo `id` in `project`.
#[tauri::command]
pub async fn todo_remove_blocker(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    id: TodoId,
    blocker: TodoId,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| f.todo_remove_blocker_in(project, id, blocker))
        .await
        .map_err(|err| err.to_string())
}

/// Adds a comment `body` to todo `id` in `project` (local-UI path). The local user drives no bound
/// session, so the core leaves the comment unattributed — authorship is the core's call, never the
/// caller's, so a to-do board comment can never carry a forged author.
#[tauri::command]
pub async fn todo_comment_create(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    id: TodoId,
    body: String,
) -> Result<TodoView, String> {
    facade
        .blocking(move |f| f.todo_comment_create_in(project, id, &body))
        .await
        .map_err(|err| err.to_string())
}

/// The `solo://` link to scratchpad `id` in `project` — for the panel's "Copy link" affordance.
#[tauri::command]
pub async fn scratchpad_link(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    scratchpad: ScratchpadId,
) -> Result<String, String> {
    Ok(facade.scratchpad_link(project, scratchpad))
}

/// The `solo://` link to todo `id` in `project` — for the board's "Copy link" affordance.
#[tauri::command]
pub async fn todo_link(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    todo: TodoId,
) -> Result<String, String> {
    Ok(facade.todo_link(project, todo))
}
