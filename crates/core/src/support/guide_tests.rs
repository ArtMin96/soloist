use super::*;

use crate::settings::McpFeatureGroup;

/// The identity-topic line that names a `whoami` refusal `reason` — the guidance an agent matches
/// its own refusal against.
fn refusal_guidance(reason: &str) -> String {
    let topic = help_topic("identity").expect("the identity topic resolves");
    topic
        .lines()
        .find(|line| line.contains(reason))
        .unwrap_or_else(|| panic!("the identity topic must explain the {reason} refusal: {topic}"))
        .to_string()
}

#[test]
fn the_guide_teaches_automatic_binding_and_names_the_manual_bind() {
    let guide = agent_guide();
    // The injected id and the external fallback are named...
    assert!(guide.contains(PROCESS_ID_ENV));
    assert!(guide.contains("register_agent"));
    assert!(guide.contains("whoami"));
    // ...binding is taught as automatic...
    assert!(guide.contains("automatically"));
    // ...and the explicit bind is named, which an agent that reads the guide but not the tool list
    // would otherwise never learn exists.
    assert!(
        guide.contains("bind_session_process"),
        "the guide must name the tool an agent binds with"
    );
}

#[test]
fn the_identity_topic_offers_the_bind_retry_only_where_it_can_succeed() {
    // `bind_session_process` re-runs the very check that produced the refusal, against the same
    // connection. For `foreign_process` — the caller does not run in the process it named — that
    // check reads a peer process group fixed for the life of the connection, so the retry can only
    // be refused again. Sending an agent back to the tool there leaves it with no remedy at all,
    // so the guidance has to say so and name what the agent can do instead.
    let foreign = refusal_guidance("foreign_process");
    assert!(
        foreign.contains("refuses the same way"),
        "the guide must say the retry is refused again rather than offer it: {foreign}"
    );
    assert!(
        foreign.contains("unbound"),
        "the guide must say what an agent that cannot bind does instead: {foreign}"
    );
    // `unknown_process` is the refusal a retry can genuinely clear — the process may register
    // after the automatic bind ran — so that is where the tool belongs.
    let unknown = refusal_guidance("unknown_process");
    assert!(
        unknown.contains("bind_session_process"),
        "the guide must offer the retry where it can succeed: {unknown}"
    );
}

#[test]
fn the_timers_topic_separates_going_quiet_from_finishing() {
    // A fire-when-idle timer fires on quiet, and quiet is not completion — a worker pausing
    // mid-task is quiet too. The guide is what an agent reads instead of the tool list, so it is
    // where a lead would otherwise learn to treat a wake as "my workers are done".
    let topic = help_topic("timers").expect("the timers topic resolves");
    assert!(
        topic.contains("not the same as"),
        "the topic must not leave quiet reading as finished: {topic}"
    );
    assert!(
        topic.contains("report_to_lead"),
        "the topic must name what completion actually looks like: {topic}"
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
