//! Per-project local settings — a per-project surface over the one settings base. The durable
//! preference record for a single project: its auto-start gate, editor override, notification
//! toggles, and per-command alert overrides. These are **app-local** preferences, stored apart from
//! the project's shared `solo.yml` config (C1, [`Visibility::Shared`](crate::config)) and never
//! written to it. The same [`SettingsStore`](crate::settings::SettingsStore) base serves this
//! surface with `K = ProjectId`, so adding a field stays one `#[serde(default)]` field plus one
//! façade getter/setter — never a new store.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ToolDefaults;

/// The per-project local settings document. Every field carries a serde default so a record an
/// older build wrote still deserializes after a field is added. Stored app-local, keyed by
/// `ProjectId`; never part of `solo.yml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSettings {
    /// When engaged (`true`), suppresses auto-start for this project: none of its commands start
    /// automatically when the project opens, regardless of each command's own `auto_start`. Off by
    /// default, so a fresh project keeps the normal behaviour (commands with `auto_start` launch on
    /// open). A project-level gate, distinct from the per-command `auto_start` in `solo.yml`.
    pub auto_start_gate: bool,
    /// Editor launch name overriding the global Tools default for this project. `None` falls back
    /// to the global default (see [`Self::resolved_editor`]).
    pub editor_override: Option<String>,
    /// Notify when a command crashes or exits unexpectedly. On by default.
    pub crash_exit_alerts: bool,
    /// Notify when a command rings the terminal bell or requests attention. On by default
    /// project-wide; a single command can be silenced via [`Self::command_terminal_alerts`].
    pub terminal_alerts: bool,
    /// Per-command terminal-alert overrides, keyed by command name. An absent command uses the
    /// project default (on); only commands the user has toggled away from the default are stored.
    pub command_terminal_alerts: BTreeMap<String, bool>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            auto_start_gate: false,
            editor_override: None,
            crash_exit_alerts: true,
            terminal_alerts: true,
            command_terminal_alerts: BTreeMap::new(),
        }
    }
}

impl ProjectSettings {
    /// The editor to open this project with: the project override when set, otherwise the global
    /// Tools default (`None` = the system default). One resolver, so "which editor" has a single
    /// source layering the per-project override over the global default.
    pub fn resolved_editor<'a>(&'a self, global: &'a ToolDefaults) -> Option<&'a str> {
        self.editor_override
            .as_deref()
            .or(global.default_editor.as_deref())
    }

    /// Whether a command's terminal alerts are on: its per-command override when set, otherwise the
    /// project-wide [`terminal_alerts`](Self::terminal_alerts) default.
    pub fn terminal_alerts_for(&self, command: &str) -> bool {
        self.command_terminal_alerts
            .get(command)
            .copied()
            .unwrap_or(self.terminal_alerts)
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
