//! Behaviour tests for the per-project settings façade methods: defaults on a fresh install, each
//! setter persisting and reading back, per-project isolation (one project's settings never leak to
//! another), and the editor resolver (project override over the global Tools default).

use std::sync::Arc;

use super::*;
use crate::ids::ProjectId;
use crate::ports::{CorePorts, TokioClock};
use crate::settings::ToolDefaults;
use crate::testing::{FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTrustRepo};

const P: ProjectId = ProjectId::from_raw(1);
const Q: ProjectId = ProjectId::from_raw(2);

fn facade_with_settings() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        // The global and per-project surfaces use the same generic fake keyed differently.
        .settings_repo(Arc::new(FakeSettingsRepo::new()))
        .project_settings_repo(Arc::new(FakeSettingsRepo::new()))
        .build(),
    )
}

#[test]
fn project_settings_read_the_defaults_on_a_fresh_install() {
    let facade = facade_with_settings();
    let settings = facade.project_settings(P).unwrap();
    assert_eq!(settings, ProjectSettings::default());
    assert!(!settings.auto_start_gate, "the gate is open by default");
    assert!(settings.crash_exit_alerts);
    assert!(settings.terminal_alerts);
}

#[test]
fn each_setter_persists_through_the_facade() {
    let facade = facade_with_settings();

    let after = facade.set_project_auto_start_gate(P, true).unwrap();
    assert!(
        after.auto_start_gate,
        "the call returns the updated settings"
    );
    assert!(facade.project_settings(P).unwrap().auto_start_gate);

    facade
        .set_project_editor_override(P, Some("zed".into()))
        .unwrap();
    assert_eq!(
        facade
            .project_settings(P)
            .unwrap()
            .editor_override
            .as_deref(),
        Some("zed")
    );

    facade.set_project_crash_exit_alerts(P, false).unwrap();
    facade.set_project_terminal_alerts(P, false).unwrap();
    let settings = facade.project_settings(P).unwrap();
    assert!(!settings.crash_exit_alerts);
    assert!(!settings.terminal_alerts);
    // The earlier writes survived the later ones (one record, independent fields).
    assert!(settings.auto_start_gate);
    assert_eq!(settings.editor_override.as_deref(), Some("zed"));
}

#[test]
fn a_per_command_alert_override_is_scoped_to_that_command() {
    let facade = facade_with_settings();
    facade.set_command_terminal_alerts(P, "Web", false).unwrap();

    let settings = facade.project_settings(P).unwrap();
    assert!(!settings.terminal_alerts_for("Web"), "the override applies");
    assert!(
        settings.terminal_alerts_for("Api"),
        "an unoverridden command keeps the project default"
    );
}

#[test]
fn settings_are_isolated_per_project() {
    let facade = facade_with_settings();
    facade.set_project_auto_start_gate(P, true).unwrap();

    assert!(facade.project_settings(P).unwrap().auto_start_gate);
    assert!(
        !facade.project_settings(Q).unwrap().auto_start_gate,
        "a second project keeps its own defaults"
    );
}

#[test]
fn clearing_the_editor_override_falls_back_to_the_global_default() {
    let facade = facade_with_settings();
    facade
        .set_tool_defaults(ToolDefaults {
            default_editor: Some("code".into()),
            default_terminal: None,
        })
        .unwrap();

    // Override set: it wins.
    facade
        .set_project_editor_override(P, Some("zed".into()))
        .unwrap();
    assert_eq!(
        facade.resolved_project_editor(P).unwrap().as_deref(),
        Some("zed")
    );

    // Override cleared: the global default applies.
    facade.set_project_editor_override(P, None).unwrap();
    assert_eq!(
        facade.resolved_project_editor(P).unwrap().as_deref(),
        Some("code")
    );

    // A project that never set an override also resolves to the global default.
    assert_eq!(
        facade.resolved_project_editor(Q).unwrap().as_deref(),
        Some("code")
    );
}
