//! Unit tests for the per-project settings document: its documented defaults, the editor resolver
//! (override over global default), the per-command terminal-alert fallback, and serde
//! backward-compatibility for a record an older build wrote.

use super::*;

#[test]
fn the_defaults_are_gate_off_and_alerts_on() {
    let settings = ProjectSettings::default();
    assert!(
        !settings.auto_start_gate,
        "the auto-start gate is open by default, preserving normal auto-start"
    );
    assert!(settings.crash_exit_alerts, "crash/exit alerts default on");
    assert!(settings.terminal_alerts, "terminal alerts default on");
    assert_eq!(settings.editor_override, None);
    assert!(settings.command_terminal_alerts.is_empty());
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
fn a_command_uses_the_project_default_until_overridden() {
    let mut settings = ProjectSettings::default();
    // No override: the project-wide default (on) applies.
    assert!(settings.terminal_alerts_for("Web"));

    // Silencing one command does not affect another.
    settings.command_terminal_alerts.insert("Web".into(), false);
    assert!(!settings.terminal_alerts_for("Web"));
    assert!(settings.terminal_alerts_for("Api"));

    // Turning the project default off flips the unoverridden command too.
    settings.terminal_alerts = false;
    assert!(!settings.terminal_alerts_for("Api"));
    // …while an explicit per-command `true` override still wins.
    settings.command_terminal_alerts.insert("Api".into(), true);
    assert!(settings.terminal_alerts_for("Api"));
}

#[test]
fn a_record_missing_a_field_deserializes_to_that_field_default() {
    // A record an older build wrote omits newer fields; serde fills them from the document default.
    let partial: ProjectSettings =
        serde_json::from_str(r#"{"auto_start_gate":true}"#).expect("parse");
    assert!(partial.auto_start_gate, "the stored field is honored");
    assert!(
        partial.crash_exit_alerts,
        "an omitted field falls back to its default (on)"
    );
    assert!(partial.terminal_alerts);

    let empty: ProjectSettings = serde_json::from_str("{}").expect("parse empty");
    assert_eq!(empty, ProjectSettings::default());
}
