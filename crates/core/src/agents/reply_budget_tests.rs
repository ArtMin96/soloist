use super::*;

/// Every provider, spelled out so that adding an [`AgentKind`] without deciding what it allows
/// leaves this list short of the enum and the omission has to be answered for in review.
const EVERY_PROVIDER: [AgentKind; 8] = [
    AgentKind::Claude,
    AgentKind::Codex,
    AgentKind::Amp,
    AgentKind::Gemini,
    AgentKind::OpenCode,
    AgentKind::Copilot,
    AgentKind::Kimi,
    AgentKind::Generic,
];

/// An agent whose environment sets nothing at all.
fn no_env(_var: &str) -> Option<String> {
    None
}

#[test]
fn every_provider_resolves_to_a_positive_budget_of_its_own() {
    for kind in EVERY_PROVIDER {
        let budget = resolve(Some(kind), no_env);
        assert!(budget.bytes > 0, "{kind:?} was left with no budget");
        assert_eq!(budget.source, BudgetSource::Provider(kind), "{kind:?}");
    }
}

#[test]
fn a_session_bound_to_no_agent_gets_the_default() {
    let budget = resolve(None, no_env);

    assert_eq!(budget.bytes, DEFAULT_REPLY_BYTES);
    assert_eq!(budget.source, BudgetSource::Default);
    assert_eq!(budget.annotation, None);
}

#[test]
fn claude_without_an_override_gets_the_ceiling_it_publishes() {
    let budget = resolve(Some(AgentKind::Claude), no_env);

    assert_eq!(budget.bytes, 100_000);
    assert_eq!(
        budget.annotation,
        Some(ListAnnotation {
            key: "anthropic/maxResultSizeChars",
            bytes: 100_000,
        })
    );
}

#[test]
fn the_agents_own_token_override_sets_claudes_budget_and_what_it_advertises() {
    let budget = resolve(Some(AgentKind::Claude), |var: &str| {
        (var == "MAX_MCP_OUTPUT_TOKENS").then(|| "5000".to_string())
    });

    assert_eq!(budget.bytes, 20_000);
    assert_eq!(
        budget.source,
        BudgetSource::AgentEnv {
            var: "MAX_MCP_OUTPUT_TOKENS"
        }
    );
    assert_eq!(
        budget.annotation,
        Some(ListAnnotation {
            key: "anthropic/maxResultSizeChars",
            bytes: 20_000,
        })
    );
}

#[test]
fn an_override_that_is_not_a_positive_number_leaves_the_published_ceiling() {
    for stated in ["abc", "0", "-3", ""] {
        let budget = resolve(Some(AgentKind::Claude), |_var: &str| {
            Some(stated.to_string())
        });

        assert_eq!(budget.bytes, 100_000, "{stated:?} changed the budget");
        assert_eq!(
            budget.source,
            BudgetSource::Provider(AgentKind::Claude),
            "{stated:?}"
        );
    }
}

#[test]
fn a_provider_that_documents_no_override_ignores_the_environment() {
    let budget = resolve(Some(AgentKind::Gemini), |_var: &str| {
        Some("5000".to_string())
    });

    assert_eq!(budget.bytes, 40_000);
    assert_eq!(budget.source, BudgetSource::Provider(AgentKind::Gemini));
}

#[test]
fn each_published_ceiling_is_the_figure_its_provider_documents() {
    assert_eq!(resolve(Some(AgentKind::Gemini), no_env).bytes, 40_000);
    assert_eq!(resolve(Some(AgentKind::Kimi), no_env).bytes, 100_000);
    assert_eq!(
        resolve(Some(AgentKind::Generic), no_env).bytes,
        DEFAULT_REPLY_BYTES
    );
}

#[test]
fn a_provider_that_documents_no_annotation_advertises_none() {
    for kind in [AgentKind::Gemini, AgentKind::Kimi, AgentKind::Generic] {
        assert_eq!(resolve(Some(kind), no_env).annotation, None, "{kind:?}");
    }
}
