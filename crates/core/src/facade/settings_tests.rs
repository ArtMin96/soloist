use std::sync::Arc;

use super::*;
use crate::ports::{CorePorts, TokioClock};
use crate::settings::{
    AgentSettings, Appearance, Binding, HotkeyAction, Integrations, McpFeatureGroup,
    ProcessCpuThreshold, Sidebar, TerminalAppearance, Theme, ToolDefaults,
};
use crate::testing::{FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTrustRepo};

fn facade_with_settings() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .settings_repo(Arc::new(FakeSettingsRepo::new()))
        .build(),
    )
}

#[test]
fn mcp_tool_groups_reads_the_defaults_on_a_fresh_install() {
    let facade = facade_with_settings();
    let groups = facade.mcp_tool_groups().unwrap();
    assert!(groups.scratchpads);
    assert!(groups.todos);
    assert!(groups.timers);
    // The G10 default: Key-Value off until the user opts in.
    assert!(!groups.key_value);
}

#[test]
fn set_mcp_tool_group_persists_through_the_facade() {
    let facade = facade_with_settings();

    let returned = facade
        .set_mcp_tool_group(McpFeatureGroup::KeyValue, true)
        .unwrap();
    assert!(
        returned.key_value,
        "the call returns the updated enablement"
    );

    assert!(
        facade.mcp_tool_groups().unwrap().key_value,
        "and a re-read sees it"
    );
}

#[test]
fn disabling_a_default_on_group_is_honored() {
    let facade = facade_with_settings();
    facade
        .set_mcp_tool_group(McpFeatureGroup::Scratchpads, false)
        .unwrap();
    assert!(!facade.mcp_tool_groups().unwrap().scratchpads);
}

#[test]
fn appearance_reads_the_defaults_on_a_fresh_install() {
    let facade = facade_with_settings();
    assert_eq!(facade.appearance().unwrap(), Appearance::default());
    assert_eq!(facade.appearance().unwrap().theme, Theme::System);
}

#[test]
fn set_appearance_persists_through_the_facade_and_leaves_other_tabs_untouched() {
    let facade = facade_with_settings();

    let appearance = Appearance {
        theme: Theme::Dark,
        terminal: TerminalAppearance {
            focus_on_click: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let returned = facade.set_appearance(appearance.clone()).unwrap();
    assert_eq!(returned, appearance, "the call returns the stored document");

    // A re-read sees the persisted appearance, and an unrelated tab keeps its defaults.
    assert_eq!(facade.appearance().unwrap(), appearance);
    assert!(
        facade.mcp_tool_groups().unwrap().scratchpads,
        "writing one tab must not disturb another"
    );
}

#[test]
fn hotkeys_remap_and_reset_all_persist_through_the_facade() {
    let facade = facade_with_settings();

    // A fresh install reports every action at its code default.
    assert!(facade.hotkeys().unwrap().iter().all(|row| row.is_default));

    let custom = Binding {
        ctrl: true,
        alt: false,
        shift: false,
        super_key: false,
        key: "J".into(),
    };
    let after = facade
        .remap_hotkey(HotkeyAction::QuickJump, custom.clone())
        .unwrap();
    let row = after
        .iter()
        .find(|r| r.action == HotkeyAction::QuickJump)
        .unwrap();
    assert_eq!(row.binding, Some(custom));
    assert!(!row.is_default, "the remapped action is no longer default");

    // The override persists across a re-read.
    let reread = facade.hotkeys().unwrap();
    assert!(
        !reread
            .iter()
            .find(|r| r.action == HotkeyAction::QuickJump)
            .unwrap()
            .is_default
    );

    // Reset-all restores every default.
    facade.reset_all_hotkeys().unwrap();
    assert!(facade.hotkeys().unwrap().iter().all(|row| row.is_default));
}

#[test]
fn each_tab_round_trips_through_the_facade_independently() {
    let facade = facade_with_settings();

    // Sidebar.
    let sidebar = Sidebar {
        hide_empty_sections: true,
        process_cpu_threshold: ProcessCpuThreshold::Pct60,
        ..Default::default()
    };
    assert_eq!(
        facade.set_sidebar_settings(sidebar.clone()).unwrap(),
        sidebar
    );
    assert_eq!(facade.sidebar_settings().unwrap(), sidebar);

    // Agents (summarization opt-in; off by default).
    assert_eq!(facade.agent_settings().unwrap(), AgentSettings::default());
    let agents = AgentSettings {
        summarizer_tool: Some("claude".into()),
        summarizer_model: Some("haiku".into()),
    };
    assert_eq!(facade.set_agent_settings(agents.clone()).unwrap(), agents);
    assert_eq!(facade.agent_settings().unwrap(), agents);

    // Tools.
    let tools = ToolDefaults {
        default_editor: Some("zed".into()),
        default_terminal: None,
    };
    assert_eq!(facade.set_tool_defaults(tools.clone()).unwrap(), tools);
    assert_eq!(facade.tool_defaults().unwrap(), tools);

    // Integrations (both master toggles default on).
    assert_eq!(
        facade.integration_settings().unwrap(),
        Integrations::default()
    );
    let integrations = Integrations {
        mcp_enabled: false,
        http_api_enabled: true,
    };
    assert_eq!(
        facade.set_integration_settings(integrations).unwrap(),
        integrations
    );
    assert_eq!(facade.integration_settings().unwrap(), integrations);

    // Every earlier tab survived the later writes (independent sub-documents, one record).
    assert_eq!(facade.sidebar_settings().unwrap(), sidebar);
    assert_eq!(facade.agent_settings().unwrap(), agents);
}
