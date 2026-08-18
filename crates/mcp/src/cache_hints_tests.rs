use rmcp::model::{ListPromptsResult, ListToolsResult, ProtocolVersion};
use serde_json::{json, Value};

use super::WithCacheHints;

/// A dated version as it arrives on the wire, for versions this SDK has no constant for.
fn version(raw: &str) -> ProtocolVersion {
    serde_json::from_value(Value::String(raw.to_owned())).expect("a dated string is a version")
}

fn tools_wire(negotiated: Option<ProtocolVersion>) -> Value {
    let result = ListToolsResult::with_all_items(Vec::new()).with_cache_hints(negotiated);
    serde_json::to_value(result).expect("a list result serializes")
}

fn prompts_wire(negotiated: Option<ProtocolVersion>) -> Value {
    let result = ListPromptsResult::with_all_items(Vec::new()).with_cache_hints(negotiated);
    serde_json::to_value(result).expect("a list result serializes")
}

#[test]
fn tools_list_carries_both_hints_for_a_peer_that_requires_them() {
    let wire = tools_wire(Some(ProtocolVersion::V_2026_07_28));

    assert_eq!(wire["resultType"], json!("complete"));
    assert_eq!(wire["ttlMs"], json!(0));
    assert_eq!(wire["cacheScope"], json!("private"));
    assert_eq!(wire["tools"], json!([]));
}

#[test]
fn prompts_list_carries_both_hints_for_a_peer_that_requires_them() {
    let wire = prompts_wire(Some(ProtocolVersion::V_2026_07_28));

    assert_eq!(wire["resultType"], json!("complete"));
    assert_eq!(wire["ttlMs"], json!(0));
    assert_eq!(wire["cacheScope"], json!("private"));
    assert_eq!(wire["prompts"], json!([]));
}

#[test]
fn a_version_past_the_one_that_introduced_the_hints_still_carries_them() {
    let wire = tools_wire(Some(version("2027-01-01")));

    assert_eq!(wire["ttlMs"], json!(0));
    assert_eq!(wire["cacheScope"], json!("private"));
}

#[test]
fn a_peer_on_an_older_version_is_sent_neither_field() {
    for older in [
        ProtocolVersion::V_2025_11_25,
        ProtocolVersion::V_2025_06_18,
        ProtocolVersion::V_2024_11_05,
    ] {
        let wire = tools_wire(Some(older.clone()));

        assert!(wire.get("ttlMs").is_none(), "ttlMs sent to {older}");
        assert!(
            wire.get("cacheScope").is_none(),
            "cacheScope sent to {older}"
        );
    }
}

#[test]
fn a_peer_that_negotiated_nothing_is_sent_neither_field() {
    for wire in [tools_wire(None), prompts_wire(None)] {
        assert!(wire.get("ttlMs").is_none());
        assert!(wire.get("cacheScope").is_none());
    }
}
