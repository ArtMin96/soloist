use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::ports::TokioClock;
use crate::settings::{
    Appearance, Binding, GlassOpacity, HotkeyAction, Integrations, McpFeatureGroup, Notifications,
    ProcessCpuThreshold, Sidebar, TerminalAppearance, Theme, ThemeAppearance, ThemeConflictPolicy,
    ThemeError, ThemeFile, ToolDefaults, DEFAULT_THEME_ID,
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
fn notification_settings_default_on_and_round_trip_through_the_facade() {
    let facade = facade_with_settings();
    // The master switch is on until the user turns it off.
    assert_eq!(
        facade.notification_settings().unwrap(),
        Notifications::default()
    );
    assert!(facade.notification_settings().unwrap().enabled);
    // Silent until the user picks a bell, so no sound is played that was never asked for.
    assert_eq!(facade.notification_settings().unwrap().bell, None);

    let off = Notifications {
        enabled: false,
        bell: Some("message".into()),
    };
    assert_eq!(facade.set_notification_settings(off.clone()).unwrap(), off);
    assert_eq!(
        facade.notification_settings().unwrap(),
        off,
        "a re-read sees the whole persisted document, switch and bell alike",
    );
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
            focus_on_click: false,
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

fn custom_theme(id: &str, name: &str, appearance: &str, accent: &str) -> ThemeFile {
    ThemeFile::from_json(&format!(
        r##"{{"version":1,"id":"{id}","name":"{name}","appearance":"{appearance}","colors":{{"accent":"{accent}"}}}}"##
    ))
    .expect("valid custom theme")
}

#[test]
fn task_shaped_theme_actions_persist_only_valid_domain_mutations() {
    let facade = facade_with_settings();

    let selected = facade
        .select_theme(ThemeAppearance::Dark, "poimandres-dark-theme")
        .expect("select built-in dark theme");
    assert_eq!(selected.selected_themes.dark, "poimandres-dark-theme");

    let rejected = facade.select_theme(ThemeAppearance::Light, "poimandres-dark-theme");
    assert!(matches!(
        rejected,
        Err(AppearanceSettingsError::Theme(
            ThemeError::UnsupportedAppearance { .. }
        ))
    ));
    assert_eq!(
        facade.appearance().unwrap().selected_themes.light,
        DEFAULT_THEME_ID,
        "a rejected action is not persisted"
    );

    let created = custom_theme("mine", "Mine", "dark", "#ABC");
    let after_create = facade.create_theme(created).expect("create custom theme");
    assert_eq!(after_create.custom_themes[0].id, "mine");
    assert_eq!(
        after_create.custom_themes[0]
            .colors
            .get(crate::settings::ThemeColorRole::Accent),
        Some("#aabbcc")
    );

    let updated = custom_theme("mine", "Mine edited", "dark", "#123456");
    let after_update = facade.update_theme(updated).expect("update custom theme");
    assert_eq!(after_update.custom_themes[0].name, "Mine edited");

    let after_import = facade
        .import_theme(
            r##"{"version":1,"id":"mine","name":"Imported","appearance":"dark","colors":{"accent":"#654321"}}"##,
            ThemeConflictPolicy::KeepBoth,
        )
        .expect("keep both import");
    assert_eq!(after_import.custom_themes[1].id, "mine-2");
}

#[test]
fn inspect_theme_normalizes_sparse_json_without_persisting_it() {
    let facade = facade_with_settings();
    let inspected = facade
        .inspect_theme(
            r##"{"version":1,"name":"Sparse","appearance":"dark","colors":{"accent":"#ABC"}}"##,
        )
        .expect("inspect sparse theme");

    assert_eq!(inspected.id, "sparse");
    assert_eq!(
        inspected
            .colors
            .get(crate::settings::ThemeColorRole::Accent),
        Some("#aabbcc")
    );
    assert!(facade.appearance().unwrap().custom_themes.is_empty());
}

#[test]
fn duplicate_remove_and_glass_actions_round_trip_through_the_facade() {
    let facade = facade_with_settings();

    let duplicated = facade
        .duplicate_theme("poimandres-dark-theme")
        .expect("duplicate built-in");
    let copy_id = "poimandres-dark-theme-copy";
    assert_eq!(duplicated.custom_themes[0].id, copy_id);

    facade
        .select_theme(ThemeAppearance::Dark, copy_id)
        .expect("select custom copy");
    let removed = facade.remove_theme(copy_id).expect("remove custom copy");
    assert!(removed.custom_themes.is_empty());
    assert_eq!(removed.selected_themes.dark, DEFAULT_THEME_ID);
    assert!(matches!(
        facade.remove_theme(DEFAULT_THEME_ID),
        Err(AppearanceSettingsError::Theme(
            ThemeError::BuiltInThemeImmutable(_)
        ))
    ));

    let opacity = facade
        .set_glass_opacity(GlassOpacity::new(65).unwrap())
        .expect("set glass opacity");
    assert_eq!(opacity.glass_opacity.get(), 65);
    assert_eq!(facade.appearance().unwrap().glass_opacity.get(), 65);
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

    // Notifications (master switch defaults on).
    assert_eq!(
        facade.notification_settings().unwrap(),
        Notifications::default()
    );
    let notifications = Notifications {
        enabled: false,
        bell: None,
    };
    assert_eq!(
        facade
            .set_notification_settings(notifications.clone())
            .unwrap(),
        notifications
    );
    assert_eq!(facade.notification_settings().unwrap(), notifications);

    // Every earlier tab survived the later writes (independent sub-documents, one record).
    assert_eq!(facade.sidebar_settings().unwrap(), sidebar);
    assert_eq!(facade.integration_settings().unwrap(), integrations);
}
