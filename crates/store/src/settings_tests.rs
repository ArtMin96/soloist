use soloist_core::{
    built_in_themes, Appearance, Assist, Binding, CursorInactiveStyle, CursorStyle, FontScale,
    FontWeight, GlassOpacity, HotkeyAction, Hotkeys, Integrations, LetterSpacing, LineHeight,
    McpFeatureGroup, McpToolGroups, Notifications, ProcessCpuThreshold, ProcessMemThreshold,
    SelectedThemes, Settings, SettingsRepo, Sidebar, TerminalAppearance, Theme, ToolDefaults,
    DEFAULT_THEME_ID,
};
use tempfile::tempdir;

use crate::SqliteStore;

/// A document with every tab set away from its default — including a hotkey **remap** and a
/// **disable**, so the override map carries both a `Some(binding)` and a `None`. The hotkey map is
/// keyed by the `HotkeyAction` enum, so this is the case that exercises enum-keyed-JSON-map
/// serialization through the real `serde_json` + SQLite path (the unit tests only parse `"{}"`).
fn fully_populated() -> Settings {
    let poimandres = built_in_themes()
        .expect("the checked-in theme catalog is valid")
        .iter()
        .find(|theme| theme.id == "poimandres-dark-theme")
        .expect("Poimandres is built in")
        .clone();
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
            selected_themes: SelectedThemes {
                light: DEFAULT_THEME_ID.into(),
                dark: poimandres.id.clone(),
            },
            custom_themes: vec![poimandres],
            glass_opacity: GlassOpacity::new(65).expect("valid opacity step"),
            interface_font_scale: FontScale::Large,
            terminal: TerminalAppearance {
                focus_on_click: false,
                copy_on_select: true,
                font_family: Some("JetBrains Mono".into()),
                font_weight: FontWeight::W500,
                bold_font_weight: FontWeight::W700,
                font_scale: FontScale::Small,
                line_height: LineHeight::Comfortable,
                letter_spacing: LetterSpacing::Wide,
                cursor_style: CursorStyle::Bar,
                cursor_inactive_style: CursorInactiveStyle::None,
                cursor_blink: false,
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
        assist: Assist {
            tool: Some("Claude".into()),
        },
        integrations: Integrations {
            mcp_enabled: false,
            http_api_enabled: false,
        },
        notifications: Notifications {
            enabled: false,
            bell: Some("bell".into()),
        },
        mcp_tool_groups,
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
fn a_record_without_the_cursor_fields_reads_back_with_the_documented_defaults() {
    // The settings document is stored as JSON, so a record an older build wrote simply lacks the
    // fields that build did not know about. Reading it must apply the documented defaults rather
    // than failing the parse — and the defaults must be the ones that preserve today's behavior,
    // so nobody's terminal changes shape on upgrade.
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    store
        .lock()
        .execute(
            "INSERT INTO settings (id, doc) VALUES (1, ?1)",
            (r#"{"appearance":{"theme":"dark","terminal":{"font_scale":"large"}}}"#,),
        )
        .expect("seed a settings record written before the cursor fields existed");

    let appearance = store
        .load(&())
        .expect("an older record still parses")
        .expect("the seeded record is found")
        .appearance;
    let terminal = appearance.terminal;

    assert_eq!(terminal.cursor_style, CursorStyle::Block);
    assert_eq!(terminal.cursor_inactive_style, CursorInactiveStyle::Outline);
    assert!(
        terminal.cursor_blink,
        "the cursor keeps blinking on upgrade — xterm defaults this off, the app never has"
    );
    // The fields the older build did write are untouched by the defaulting.
    assert_eq!(terminal.font_scale, FontScale::Large);
    assert_eq!(appearance.selected_themes.light, DEFAULT_THEME_ID);
    assert_eq!(appearance.selected_themes.dark, DEFAULT_THEME_ID);
    assert_eq!(appearance.glass_opacity.get(), 80);
    assert!(appearance.custom_themes.is_empty());
}

#[test]
fn a_record_without_the_terminal_behavior_fields_reads_back_with_the_documented_defaults() {
    // The two terminal behavior booleans are read by the emulator, so their defaults decide what a
    // user who never touched them gets. A record an older build wrote simply lacks them, and the
    // defaults that apply must be the ones that leave the terminal behaving as it always has:
    // selecting a process focuses it, and a selection is copied only on the explicit hotkey.
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    store
        .lock()
        .execute(
            "INSERT INTO settings (id, doc) VALUES (1, ?1)",
            (r#"{"appearance":{"terminal":{"letter_spacing":"wide"}}}"#,),
        )
        .expect("seed a settings record written before the terminal behavior fields existed");

    let terminal = store
        .load(&())
        .expect("an older record still parses")
        .expect("the seeded record is found")
        .appearance
        .terminal;

    assert!(
        terminal.focus_on_click,
        "selecting a process keeps focusing its terminal on upgrade"
    );
    assert!(
        !terminal.copy_on_select,
        "a selection is not copied until the user opts in"
    );
    // The field the older build did write is untouched by the defaulting.
    assert_eq!(terminal.letter_spacing, LetterSpacing::Wide);
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
