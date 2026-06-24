use std::sync::Arc;

use super::*;
use crate::ports::{CorePorts, TokioClock};
use crate::settings::McpFeatureGroup;
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
