//! Unit tests for the settings aggregate and its document — defaults (Key-Value off, the rest on),
//! per-group enablement, default-on-absent, persistence through a fake repo, and serde
//! backward-compatibility for a record an older build wrote.

use std::sync::Arc;

use super::*;
use crate::testing::FakeSettingsRepo;

#[test]
fn the_mcp_tool_group_defaults_serve_every_feature_group_except_key_value() {
    let groups = McpToolGroups::default();

    assert!(groups.enabled(McpFeatureGroup::Scratchpads));
    assert!(groups.enabled(McpFeatureGroup::Todos));
    assert!(groups.enabled(McpFeatureGroup::Timers));
    // Key-Value is the one feature group that defaults off.
    assert!(!groups.enabled(McpFeatureGroup::KeyValue));
}

#[test]
fn enabled_reflects_set_for_every_group() {
    for group in McpFeatureGroup::ALL {
        let mut groups = McpToolGroups::default();

        groups.set(group, true);
        assert!(
            groups.enabled(group),
            "{group:?} should read enabled after set(true)"
        );

        groups.set(group, false);
        assert!(
            !groups.enabled(group),
            "{group:?} should read disabled after set(false)"
        );
    }
}

#[test]
fn an_empty_store_reads_the_defaults() {
    let store = SettingsStore::new(Arc::new(FakeSettingsRepo::new()));

    let groups = store.mcp_tool_groups().expect("read defaults");

    assert_eq!(groups, McpToolGroups::default());
    assert!(!groups.key_value);
}

#[test]
fn a_noop_repo_keeps_the_defaults_even_after_a_write() {
    // The Noop port discards writes, so the core runs at the defaults without a durable adapter.
    let store = SettingsStore::new(Arc::new(NoopSettingsRepo));

    store
        .set_mcp_tool_group(McpFeatureGroup::KeyValue, true)
        .expect("Noop write succeeds");

    assert!(!store.mcp_tool_groups().expect("re-read").key_value);
}

#[test]
fn set_mcp_tool_group_persists_and_reads_back() {
    let store = SettingsStore::new(Arc::new(FakeSettingsRepo::new()));

    let returned = store
        .set_mcp_tool_group(McpFeatureGroup::KeyValue, true)
        .expect("enable key-value");
    assert!(
        returned.key_value,
        "the call returns the updated enablement"
    );

    // A fresh read sees the persisted change, and the untouched groups keep their defaults.
    let groups = store.mcp_tool_groups().expect("re-read");
    assert!(groups.key_value);
    assert!(groups.scratchpads);
    assert!(groups.todos);
    assert!(groups.timers);
}

#[test]
fn turning_a_default_on_group_off_persists() {
    let store = SettingsStore::new(Arc::new(FakeSettingsRepo::new()));

    store
        .set_mcp_tool_group(McpFeatureGroup::Todos, false)
        .expect("disable todos");

    assert!(!store.mcp_tool_groups().expect("re-read").todos);
}

#[test]
fn a_record_missing_a_field_deserializes_to_the_default_for_that_field() {
    // A record an older build wrote omits a newer field; serde fills it from the document default,
    // so an absent `key_value` reads as off and an empty document reads as the full defaults.
    let partial: Settings =
        serde_json::from_str(r#"{"mcp_tool_groups":{"scratchpads":false}}"#).expect("parse");
    assert!(
        !partial.mcp_tool_groups.scratchpads,
        "the stored field is honored"
    );
    assert!(
        partial.mcp_tool_groups.todos,
        "an omitted field falls back to its default"
    );
    assert!(!partial.mcp_tool_groups.key_value);

    let empty: Settings = serde_json::from_str("{}").expect("parse empty");
    assert_eq!(empty, Settings::default());
}
