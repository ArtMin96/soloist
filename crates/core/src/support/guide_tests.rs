use super::*;

use crate::settings::McpFeatureGroup;

#[test]
fn the_guide_teaches_automatic_binding_not_a_manual_bind_call() {
    let guide = agent_guide();
    // The injected id and the external fallback are named...
    assert!(guide.contains(PROCESS_ID_ENV));
    assert!(guide.contains("register_agent"));
    assert!(guide.contains("whoami"));
    // ...and binding is taught as automatic — there is no bind tool for an agent to call, so the
    // guide must not instruct one (the earlier text told agents to call `bind_session_process`).
    assert!(guide.contains("automatically"));
    assert!(
        !guide.contains("bind_session_process"),
        "the guide must not tell agents to call a bind tool that the MCP surface does not expose"
    );
}

#[test]
fn the_guide_covers_scope_trust_and_idle_timers() {
    let guide = agent_guide();
    assert!(guide.contains("select_project"));
    assert!(guide.contains("untrusted"));
    assert!(guide.contains("timer_fire_when_idle_any"));
    assert!(guide.contains("wait_for_bound_port"));
}

#[test]
fn the_full_guide_lists_every_toggleable_group() {
    let guide = agent_guide();
    for group in McpFeatureGroup::ALL {
        assert!(
            guide.contains(&group.label().to_lowercase()),
            "the guide must name the {} group",
            group.label()
        );
    }
}

#[test]
fn the_guide_prescribes_coordination_etiquette() {
    let guide = agent_guide();
    assert!(guide.contains("lock_acquire"));
    assert!(guide.contains("todo_lock"));
    assert!(guide.contains("revision"));
}

#[test]
fn the_overview_is_compact_and_advertises_the_onboarding_path() {
    let overview = help_overview();
    // The overview names the first-run tools and lists topics without dumping every topic body,
    // so it stays shorter than the full guide it is a menu for.
    assert!(overview.contains("whoami"));
    assert!(overview.contains("`timers`"));
    assert!(overview.contains("`identity`"));
    assert!(
        overview.len() < agent_guide().len(),
        "the overview is a menu, not the whole guide"
    );
}

#[test]
fn a_topic_resolves_by_key_and_by_alias() {
    // A canonical key resolves...
    let by_key = help_topic("timers").expect("the timers topic resolves by key");
    assert!(by_key.contains("timer_fire_when_idle_any"));
    // ...and an alias resolves to the same section.
    assert_eq!(help_topic("idle").as_deref(), Some(by_key.as_str()));
}

#[test]
fn topic_lookup_normalizes_separators_and_case() {
    // The tweet's example aliases all route to a topic regardless of spelling.
    for query in [
        "ports", "services", "status", "how do I", "How-Do-I", "yaml",
    ] {
        assert!(
            help_topic(query).is_some(),
            "the query {query:?} should resolve to a topic"
        );
    }
}

#[test]
fn an_unknown_topic_does_not_resolve() {
    assert!(help_topic("there-is-no-such-topic").is_none());
}

#[test]
fn the_onboarding_hint_is_the_overviews_first_run_path() {
    assert!(help_overview().contains(onboarding_hint()));
}
