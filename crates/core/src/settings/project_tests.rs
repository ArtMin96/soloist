//! Unit tests for the per-project settings document: its documented defaults, the editor resolver
//! (override over global default), the per-command level fallback, and serde
//! backward-compatibility for a record an older build wrote.

use super::*;

#[test]
fn the_default_gate_is_off_with_no_overrides() {
    let settings = ProjectSettings::default();
    assert!(
        !settings.auto_start_gate,
        "the auto-start gate is open by default, preserving normal auto-start"
    );
    assert_eq!(settings.editor_override, None);
    assert!(settings.command_notification_levels.is_empty());
}

#[test]
fn absent_settings_default_to_all() {
    assert_eq!(
        ProjectSettings::default().notification_level,
        NotificationLevel::All,
        "a fresh project notifies about everything, matching the alerts-on default it replaces"
    );
}

#[test]
fn the_editor_override_wins_over_the_global_default() {
    let global = ToolDefaults {
        default_editor: Some("code".into()),
        default_terminal: None,
    };
    let settings = ProjectSettings {
        editor_override: Some("zed".into()),
        ..Default::default()
    };
    assert_eq!(settings.resolved_editor(&global), Some("zed"));
}

#[test]
fn the_editor_resolves_to_the_global_default_when_no_override() {
    let global = ToolDefaults {
        default_editor: Some("code".into()),
        default_terminal: None,
    };
    let settings = ProjectSettings::default();
    assert_eq!(settings.resolved_editor(&global), Some("code"));
}

#[test]
fn the_editor_is_none_when_neither_override_nor_global_is_set() {
    let settings = ProjectSettings::default();
    assert_eq!(settings.resolved_editor(&ToolDefaults::default()), None);
}

#[test]
fn a_command_inherits_the_project_level_until_overridden() {
    let mut settings = ProjectSettings::default();
    assert_eq!(
        settings.effective_level_for("Web"),
        NotificationLevel::All,
        "with no entry of its own a command is exactly as loud as its project"
    );

    settings
        .command_notification_levels
        .insert("Web".into(), NotificationLevel::None);
    assert_eq!(settings.effective_level_for("Web"), NotificationLevel::None);
    assert_eq!(
        settings.effective_level_for("Api"),
        NotificationLevel::All,
        "silencing one command leaves the others alone"
    );
}

#[test]
fn a_command_cannot_be_louder_than_its_project() {
    let mut settings = ProjectSettings {
        notification_level: NotificationLevel::Important,
        ..Default::default()
    };
    settings
        .command_notification_levels
        .insert("Web".into(), NotificationLevel::All);

    assert_eq!(
        settings.effective_level_for("Web"),
        NotificationLevel::Important,
        "the project and the command combine to the tighter of the two"
    );
}

#[test]
fn legacy_booleans_upgrade_to_a_level() {
    // A record an older build wrote carries the two alert booleans and no level. All four
    // combinations map to a level; "crashes off, bells on" has no equivalent and resolves to the
    // louder side, because an unwanted alert is one click to fix and a missed crash is not.
    let cases = [
        (true, true, NotificationLevel::All),
        (true, false, NotificationLevel::Important),
        (false, false, NotificationLevel::None),
        (false, true, NotificationLevel::All),
    ];

    for (crash_exit_alerts, terminal_alerts, level) in cases {
        let legacy = format!(
            r#"{{"crash_exit_alerts":{crash_exit_alerts},"terminal_alerts":{terminal_alerts}}}"#
        );
        let upgraded: ProjectSettings = serde_json::from_str(&legacy).expect("parse legacy record");
        assert_eq!(
            upgraded.notification_level, level,
            "crash {crash_exit_alerts} + terminal {terminal_alerts}"
        );
    }
}

#[test]
fn a_legacy_command_override_upgrades_with_the_project_crash_setting() {
    // A per-command boolean is only half of a pair; the project's crash setting supplies the other
    // half, so a silenced command under a project that still wants crashes lands on `Important`.
    let legacy = r#"{
        "crash_exit_alerts": true,
        "terminal_alerts": true,
        "command_terminal_alerts": { "Web": false, "Api": true }
    }"#;
    let upgraded: ProjectSettings = serde_json::from_str(legacy).expect("parse legacy record");

    assert_eq!(upgraded.notification_level, NotificationLevel::All);
    assert_eq!(
        upgraded.effective_level_for("Web"),
        NotificationLevel::Important,
        "the command that had bells off keeps its crashes"
    );
    assert_eq!(upgraded.effective_level_for("Api"), NotificationLevel::All);
}

#[test]
fn a_stored_level_wins_over_any_legacy_booleans() {
    // A record written after the upgrade already carries a level; a stale boolean left beside it
    // must not re-decide what the user has since chosen.
    let mixed = r#"{"notification_level":"none","crash_exit_alerts":true,"terminal_alerts":true}"#;
    let settings: ProjectSettings = serde_json::from_str(mixed).expect("parse");
    assert_eq!(settings.notification_level, NotificationLevel::None);
}

#[test]
fn a_record_missing_a_field_deserializes_to_that_field_default() {
    // A record an older build wrote omits newer fields; serde fills them from the document default.
    let partial: ProjectSettings =
        serde_json::from_str(r#"{"auto_start_gate":true}"#).expect("parse");
    assert!(partial.auto_start_gate, "the stored field is honored");
    assert_eq!(
        partial.notification_level,
        NotificationLevel::All,
        "an omitted level falls back to its default"
    );

    let empty: ProjectSettings = serde_json::from_str("{}").expect("parse empty");
    assert_eq!(empty, ProjectSettings::default());
}

fn spec(command: &str) -> crate::config::ProcessSpec {
    crate::config::ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: true,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: Default::default(),
    }
}

#[test]
fn renaming_a_command_moves_its_notification_override() {
    let mut settings = ProjectSettings::default();
    settings
        .command_notification_levels
        .insert("Web".into(), NotificationLevel::None);

    settings.rename_command("Web", "WebApp");

    assert_eq!(
        settings.effective_level_for("WebApp"),
        NotificationLevel::None,
        "the override followed the command to its new name"
    );
    assert_eq!(
        settings.effective_level_for("Web"),
        NotificationLevel::All,
        "the old name no longer carries any override"
    );
}

#[test]
fn renaming_a_local_command_moves_its_local_commands_entry_too() {
    let mut settings = ProjectSettings::default();
    settings
        .local_commands
        .insert("Logs".into(), spec("tail -f log"));

    settings.rename_command("Logs", "AppLogs");

    assert!(settings.local_commands.contains_key("AppLogs"));
    assert!(!settings.local_commands.contains_key("Logs"));
}

#[test]
fn renaming_onto_a_name_with_a_stale_override_lets_the_moved_override_win() {
    let mut settings = ProjectSettings::default();
    // "Old" is a stale entry left behind by a different, since-removed command.
    settings
        .command_notification_levels
        .insert("Old".into(), NotificationLevel::Important);
    settings
        .command_notification_levels
        .insert("Api".into(), NotificationLevel::None);

    settings.rename_command("Api", "Old");

    assert_eq!(
        settings.effective_level_for("Old"),
        NotificationLevel::None,
        "the surviving command's own override wins over a stale one at the destination name"
    );
}

#[test]
fn renaming_a_command_to_its_own_name_is_a_no_op() {
    let mut settings = ProjectSettings::default();
    settings
        .command_notification_levels
        .insert("Web".into(), NotificationLevel::None);

    settings.rename_command("Web", "Web");

    assert_eq!(settings.effective_level_for("Web"), NotificationLevel::None);
}

#[test]
fn forgetting_a_command_drops_its_notification_override() {
    let mut settings = ProjectSettings::default();
    settings
        .command_notification_levels
        .insert("Web".into(), NotificationLevel::None);

    settings.forget_command("Web");

    assert_eq!(
        settings.effective_level_for("Web"),
        NotificationLevel::All,
        "a forgotten command has no override left to inherit"
    );
}

#[test]
fn a_written_record_reads_back_unchanged() {
    // The document serializes under its own field names but deserializes through the upgrade
    // representation, so a name that drifts between the two would silently drop a setting.
    let mut settings = ProjectSettings {
        auto_start_gate: true,
        auto_trust_command_changes: true,
        editor_override: Some("zed".into()),
        notification_level: NotificationLevel::Important,
        ..Default::default()
    };
    settings
        .command_notification_levels
        .insert("Web".into(), NotificationLevel::None);

    let json = serde_json::to_string(&settings).expect("serialize");
    let read_back: ProjectSettings = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(read_back, settings);
}
