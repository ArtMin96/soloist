use soloist_core::{
    Appearance, Binding, FontScale, FontWeight, HotkeyAction, Hotkeys, Integrations, LetterSpacing,
    LineHeight, McpFeatureGroup, McpToolGroups, Notifications, ProcessCpuThreshold,
    ProcessMemThreshold, Settings, SettingsRepo, Sidebar, TemplateDefaults, TemplateId,
    TerminalAppearance, Theme, ToolDefaults,
};
use tempfile::tempdir;

use crate::SqliteStore;

/// A document with every tab set away from its default — including a hotkey **remap** and a
/// **disable**, so the override map carries both a `Some(binding)` and a `None`. The hotkey map is
/// keyed by the `HotkeyAction` enum, so this is the case that exercises enum-keyed-JSON-map
/// serialization through the real `serde_json` + SQLite path (the unit tests only parse `"{}"`).
fn fully_populated() -> Settings {
    let mut hotkeys = Hotkeys::default();
    hotkeys.remap(
        HotkeyAction::QuickJump,
        Binding {
            ctrl: true,
            alt: false,
            shift: true,
            super_key: false,
            key: "J".into(),
        },
    );
    hotkeys.disable(HotkeyAction::OpenTerminalSearch);

    let mut mcp_tool_groups = McpToolGroups::default();
    mcp_tool_groups.set(McpFeatureGroup::KeyValue, true);

    Settings {
        appearance: Appearance {
            theme: Theme::Dark,
            interface_font_scale: FontScale::Large,
            terminal: TerminalAppearance {
                focus_on_click: true,
                font_family: Some("JetBrains Mono".into()),
                font_weight: FontWeight::W500,
                bold_font_weight: FontWeight::W700,
                font_scale: FontScale::Small,
                line_height: LineHeight::Comfortable,
                letter_spacing: LetterSpacing::Wide,
            },
        },
        sidebar: Sidebar {
            show_filter_input: false,
            hide_empty_sections: true,
            process_cpu_threshold: ProcessCpuThreshold::Pct60,
            process_mem_threshold: ProcessMemThreshold::Mb500,
            show_settings_footer: false,
        },
        hotkeys,
        tools: ToolDefaults {
            default_editor: Some("zed".into()),
            default_terminal: Some("kitty".into()),
        },
        integrations: Integrations {
            mcp_enabled: false,
            http_api_enabled: false,
        },
        notifications: Notifications { enabled: false },
        mcp_tool_groups,
        template_defaults: TemplateDefaults {
            scratchpad: Some(TemplateId::from_raw(3)),
            todo: None,
        },
    }
}

/// A settings document with Key-Value turned on — distinct from the defaults, so a round-trip
/// proves the stored record (not the default) came back.
fn key_value_enabled() -> Settings {
    let mut settings = Settings::default();
    settings
        .mcp_tool_groups
        .set(McpFeatureGroup::KeyValue, true);
    settings
}

#[test]
fn load_on_a_fresh_store_returns_none() {
    // Nothing stored yet, so the aggregate applies the documented defaults.
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    assert_eq!(store.load(&()).unwrap(), None);
}

#[test]
fn save_replaces_the_single_record() {
    // The `id = 1` singleton: a second save overwrites the first rather than adding a row.
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    store.save(&(), &Settings::default()).unwrap();
    store.save(&(), &key_value_enabled()).unwrap();

    assert_eq!(store.load(&()).unwrap(), Some(key_value_enabled()));
    let count: i64 = store
        .lock()
        .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1, "the settings table holds exactly one record");
}

#[test]
fn settings_survive_a_store_reopen() {
    // Settings are durable global content: they persist across an app restart. Save, reopen on the
    // same file, and read the stored record back.
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let settings = key_value_enabled();
    {
        let store = SqliteStore::open(&db).expect("open");
        store.save(&(), &settings).unwrap();
    }

    let store = SqliteStore::open(&db).expect("reopen");
    assert_eq!(
        store.load(&()).unwrap(),
        Some(settings),
        "the settings record survives the reopen"
    );
}

#[test]
fn a_fully_populated_document_round_trips_through_the_real_store() {
    // Every tab away from its defaults, including the enum-keyed hotkey override map, must persist
    // and read back byte-for-byte through real serde_json + SQLite — not just the one-field record.
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    let settings = fully_populated();
    store.save(&(), &settings).unwrap();
    assert_eq!(store.load(&()).unwrap(), Some(settings));
}

#[test]
fn a_fully_populated_document_survives_a_store_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let settings = fully_populated();
    {
        let store = SqliteStore::open(&db).expect("open");
        store.save(&(), &settings).unwrap();
    }

    let store = SqliteStore::open(&db).expect("reopen");
    assert_eq!(
        store.load(&()).unwrap(),
        Some(settings),
        "every tab — including the hotkey overrides — survives the reopen"
    );
}
