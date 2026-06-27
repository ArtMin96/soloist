//! The per-project settings page read-model (context C8 → projects): one assembled view of a
//! project's settings surface — its root, config validity, command roster, and live counts — that
//! the settings page renders directly.
//!
//! A pure projection: the command fields are **flattened** (not a nested [`ProcessSpec`]) so every
//! field is always present in JSON even when the spec leaves it at its default, and each command
//! carries its [`Visibility`] so the page shows whether the command lives in the shared `solo.yml`
//! (committed) or the app-local overlay (this machine only).

use serde::{Deserialize, Serialize};

use crate::config::ProcessSpec;
use crate::ids::ProjectId;
use crate::process::ProcStatus;
use crate::settings::ProjectSettings;

/// Where a command lives: in the shared `solo.yml` ([`Shared`](Visibility::Shared), committed) or in
/// the app-local overlay ([`Local`](Visibility::Local), this machine only). A closed enum, so every
/// consumer handles both cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Shared,
    Local,
}

/// One command on the settings page. Its spec fields are flattened so they are always present in
/// JSON (a [`ProcessSpec`] omits fields left at their defaults); [`visibility`](Self::visibility)
/// is where it lives; [`terminal_alerts`](Self::terminal_alerts) is its resolved alert state; and
/// [`status`](Self::status) is the live [`ProcStatus`] when a process of that name is registered,
/// else `None`.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectCommandView {
    pub name: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub auto_start: bool,
    pub auto_restart: bool,
    pub restart_when_changed: Vec<String>,
    pub visibility: Visibility,
    pub terminal_alerts: bool,
    pub status: Option<ProcStatus>,
}

impl ProjectCommandView {
    /// Projects a named command's spec into the flattened page view, tagging its visibility, its
    /// resolved terminal-alert state, and its live status (if any).
    pub fn new(
        name: String,
        spec: &ProcessSpec,
        visibility: Visibility,
        terminal_alerts: bool,
        status: Option<ProcStatus>,
    ) -> Self {
        Self {
            name,
            command: spec.command.clone(),
            working_dir: spec
                .working_dir
                .as_ref()
                .map(|dir| dir.to_string_lossy().into_owned()),
            auto_start: spec.auto_start,
            auto_restart: spec.auto_restart,
            restart_when_changed: spec.restart_when_changed.clone(),
            visibility,
            terminal_alerts,
            status,
        }
    }
}

/// Whether the project's `solo.yml` currently loads. `error` carries the parser/IO message when it
/// does not, so the page can surface exactly why the config is broken.
#[derive(Clone, Debug, Serialize)]
pub struct ConfigStatus {
    pub valid: bool,
    pub error: Option<String>,
}

/// The assembled per-project settings page: the project's id and root, its config validity, the live
/// running/total command counts, its local settings, the resolved editor, and the full command
/// roster (shared then local). One read the settings page renders directly.
#[derive(Clone, Debug, Serialize)]
pub struct ProjectSettingsPage {
    pub project: ProjectId,
    pub root: String,
    pub config: ConfigStatus,
    pub running: usize,
    pub total: usize,
    pub settings: ProjectSettings,
    pub resolved_editor: Option<String>,
    pub commands: Vec<ProjectCommandView>,
}
