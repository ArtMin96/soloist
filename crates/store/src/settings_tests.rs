use soloist_core::{McpFeatureGroup, Settings, SettingsRepo};
use tempfile::tempdir;

use crate::SqliteStore;

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
fn save_then_load_round_trips() {
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    let settings = key_value_enabled();
    store.save(&(), &settings).unwrap();
    assert_eq!(store.load(&()).unwrap(), Some(settings));
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
