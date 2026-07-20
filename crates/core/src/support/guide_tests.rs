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
fn every_topic_is_rendered_into_the_full_guide_and_the_overview_menu() {
    // The topic set is the single source for all three renderings, so none may be silently dropped:
    // the full guide must carry every topic's section, and the overview must list every topic key.
    let guide = agent_guide();
    let overview = help_overview();
    for topic in topics() {
        assert!(
            guide.contains(&topic.rendered()),
            "the full guide must render the {} topic",
            topic.key
        );
        assert!(
            overview.contains(&format!("`{}`", topic.key)),
            "the overview menu must list the {} topic",
            topic.key
        );
    }
}

#[test]
fn every_topic_resolves_by_its_key_and_aliases_and_renders_a_body() {
    // What each smoke gestured at, made structural: every registered topic is reachable by its key
    // and by every alias it declares, and none renders an empty section.
    for topic in topics() {
        let rendered = topic.rendered();
        assert_eq!(
            help_topic(topic.key).as_deref(),
            Some(rendered.as_str()),
            "the {} topic resolves by its own key",
            topic.key
        );
        assert!(
            !topic.body.trim().is_empty(),
            "the {} topic renders a non-empty body",
            topic.key
        );
        for alias in topic.aliases {
            assert_eq!(
                help_topic(alias).as_deref(),
                Some(rendered.as_str()),
                "the alias {alias:?} resolves to the {} topic",
                topic.key
            );
        }
    }
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
fn the_prompt_templates_topic_resolves_by_key_and_by_every_alias() {
    // Prompt templates is the only toggleable group that defaults off, so an agent's only in-band
    // way to learn it exists is this topic — which the overview already advertises the group in.
    // Each alias is asserted on its own: the loop over declared aliases above cannot catch an alias
    // that was never declared, and `help(topic="prompt templates")` is the query that found nothing.
    let by_key =
        help_topic("prompt-templates").expect("the prompt-templates topic resolves by key");
    assert!(by_key.contains("prompt_template_render"));
    for query in [
        "prompt templates",
        "prompt",
        "prompts",
        "template",
        "templates",
    ] {
        assert_eq!(
            help_topic(query).as_deref(),
            Some(by_key.as_str()),
            "the query {query:?} resolves to the prompt-templates topic"
        );
    }
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
