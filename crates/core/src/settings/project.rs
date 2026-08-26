//! Per-project local settings — a per-project surface over the one settings base. The durable
//! preference record for a single project: its auto-start gate, editor override, notification
//! level, per-command level overrides, and default templates. These are **app-local**
//! preferences, stored apart from the project's shared `solo.yml` config (C1,
//! [`Visibility::Shared`](crate::projects::Visibility)) and never
//! written to it. The same [`SettingsStore`](crate::settings::SettingsStore) base serves this
//! surface with `K = ProjectId`, so adding a field stays one `#[serde(default)]` field plus one
//! façade getter/setter — never a new store.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::{NotificationLevel, TemplateDefaults, ToolDefaults};
use crate::config::ProcessSpec;

/// The per-project local settings document. Every field carries a serde default so a record an
/// older build wrote still deserializes after a field is added, and it reads through
/// [`StoredProjectSettings`] so a record written before the notification level existed upgrades to
/// one. Stored app-local, keyed by `ProjectId`; never part of `solo.yml`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredProjectSettings")]
pub struct ProjectSettings {
    /// When engaged (`true`), suppresses auto-start for this project: none of its commands start
    /// automatically when the project opens, regardless of each command's own `auto_start`. Off by
    /// default, so a fresh project keeps the normal behaviour (commands with `auto_start` launch on
    /// open). A project-level gate, distinct from the per-command `auto_start` in `solo.yml`.
    pub auto_start_gate: bool,
    /// When enabled (`true`), a command the user creates or edits in this project's `solo.yml`
    /// **through Soloist** is trusted automatically, so it can start without a separate trust
    /// prompt. Applies only to user-initiated saves; a change made to `solo.yml` outside Soloist
    /// still syncs in untrusted and requires explicit trust. Off by default, so trust stays
    /// explicit unless the user opts in.
    pub auto_trust_command_changes: bool,
    /// Editor launch name overriding the global Tools default for this project. `None` falls back
    /// to the global default (see [`Self::resolved_editor`]).
    pub editor_override: Option<String>,
    /// How much this project notifies. [`All`](NotificationLevel::All) by default, so a fresh
    /// project raises every alert; a single command can be quietened via
    /// [`Self::command_notification_levels`].
    pub notification_level: NotificationLevel,
    /// Per-command level overrides, keyed by command name. An absent command inherits the project
    /// level; a present one combines with it, so an override can only tighten (see
    /// [`Self::effective_level_for`]). Keyed by a mutable name, so a rename or removal must route
    /// through [`Self::rename_command`]/[`Self::forget_command`] rather than mutating this map
    /// directly, or the entry is stranded under a name nothing uses any more.
    pub command_notification_levels: BTreeMap<String, NotificationLevel>,
    /// App-local commands — managed processes kept on this machine only, **never** written to
    /// `solo.yml` (`Visibility::Local`). Same shape as a shared command, keyed by name in display
    /// order. The "Make local" / "Save to solo.yml" move transfers a command between this overlay
    /// and the shared config; the two stores never hold the same command at once after a move.
    pub local_commands: IndexMap<String, ProcessSpec>,
    /// The template a new empty document of each seedable kind starts from, selected from **this
    /// project's** template library. Nothing selected (the default) seeds an empty document.
    pub template_defaults: TemplateDefaults,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            auto_start_gate: false,
            auto_trust_command_changes: false,
            editor_override: None,
            notification_level: NotificationLevel::All,
            command_notification_levels: BTreeMap::new(),
            local_commands: IndexMap::new(),
            template_defaults: TemplateDefaults::default(),
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

    /// How much a single command notifies: its own override combined with the project level,
    /// taking the more restrictive of the two, so a command can go quieter than its project but
    /// never louder. An unoverridden command is exactly as loud as its project.
    pub fn effective_level_for(&self, command: &str) -> NotificationLevel {
        self.command_notification_levels
            .get(command)
            .copied()
            .map_or(self.notification_level, |command_level| {
                self.notification_level.most_restrictive(command_level)
            })
    }

    /// Moves this command's per-command settings state — its notification override and, if it is a
    /// local command, its `local_commands` entry — from `from` to `to`, so a rename carries every
    /// name-keyed fact along in one place rather than stranding some of it under the old name. A
    /// no-op when `from` and `to` are the same. If `to` already carries an override (only reachable
    /// when a different, since-removed command once held it), the moved override replaces it: it is
    /// the active choice of the command that survives the rename.
    pub fn rename_command(&mut self, from: &str, to: &str) {
        if from == to {
            return;
        }
        if let Some(level) = self.command_notification_levels.remove(from) {
            self.command_notification_levels
                .insert(to.to_owned(), level);
        }
        if let Some(idx) = self.local_commands.get_index_of(from) {
            if let Some((_, spec)) = self.local_commands.shift_remove_index(idx) {
                self.local_commands.shift_insert(idx, to.to_owned(), spec);
            }
        }
    }

    /// Drops this command's notification override, so a later, unrelated command created under the
    /// same name starts at the project level instead of silently inheriting a retired command's
    /// choice.
    pub fn forget_command(&mut self, name: &str) {
        self.command_notification_levels.remove(name);
    }
}

/// The stored shape of [`ProjectSettings`], read on the way in so an older record upgrades. It
/// carries the retired `crash_exit_alerts` / `terminal_alerts` pair alongside the level that
/// replaced them; a record holding only the booleans is mapped by [`level_from_legacy_alerts`].
/// The document's serde defaults live here, because `#[serde(from)]` bypasses them on
/// [`ProjectSettings`] itself.
#[derive(Default, Deserialize)]
#[serde(default)]
struct StoredProjectSettings {
    auto_start_gate: bool,
    auto_trust_command_changes: bool,
    editor_override: Option<String>,
    notification_level: Option<NotificationLevel>,
    command_notification_levels: BTreeMap<String, NotificationLevel>,
    crash_exit_alerts: Option<bool>,
    terminal_alerts: Option<bool>,
    command_terminal_alerts: BTreeMap<String, bool>,
    local_commands: IndexMap<String, ProcessSpec>,
    template_defaults: TemplateDefaults,
}

impl From<StoredProjectSettings> for ProjectSettings {
    fn from(stored: StoredProjectSettings) -> Self {
        let crash_exit_alerts = stored.crash_exit_alerts.unwrap_or(true);
        let terminal_alerts = stored.terminal_alerts.unwrap_or(true);
        let notification_level = stored
            .notification_level
            .unwrap_or_else(|| level_from_legacy_alerts(crash_exit_alerts, terminal_alerts));

        let mut command_notification_levels = stored.command_notification_levels;
        for (command, command_terminal_alerts) in stored.command_terminal_alerts {
            command_notification_levels
                .entry(command)
                .or_insert_with(|| {
                    level_from_legacy_alerts(crash_exit_alerts, command_terminal_alerts)
                });
        }

        Self {
            auto_start_gate: stored.auto_start_gate,
            auto_trust_command_changes: stored.auto_trust_command_changes,
            editor_override: stored.editor_override,
            notification_level,
            command_notification_levels,
            local_commands: stored.local_commands,
            template_defaults: stored.template_defaults,
        }
    }
}

/// The level a retired pair of alert booleans becomes. Three of the four combinations have an exact
/// equivalent; "crashes off, bells on" has none, and resolves to the louder side — an unwanted
/// alert is one click for the user to undo, while a crash that never announced itself is neither
/// noticed nor recoverable.
fn level_from_legacy_alerts(crash_exit_alerts: bool, terminal_alerts: bool) -> NotificationLevel {
    match (crash_exit_alerts, terminal_alerts) {
        (true, true) | (false, true) => NotificationLevel::All,
        (true, false) => NotificationLevel::Important,
        (false, false) => NotificationLevel::None,
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
