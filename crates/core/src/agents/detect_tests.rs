use std::sync::Arc;

use super::*;
use crate::testing::{FakeAgentToolRepo, FakeVersionProbe, MockClock};

fn tool(name: &str, command: &str, kind: AgentKind) -> AgentTool {
    AgentTool {
        name: name.to_string(),
        command: command.to_string(),
        default_args: Vec::new(),
        kind,
        prompt_mode: PromptMode::AppendedArg,
    }
}

fn detection(detected: &[DetectedTool], name: &str) -> Detection {
    detected
        .iter()
        .find(|d| d.tool.name == name)
        .map(|d| d.detection)
        .expect("tool present in detection result")
}

#[tokio::test]
async fn detection_flags_the_installed_built_in_providers() {
    let tools = vec![
        tool("Claude", "claude", AgentKind::Claude),
        tool("Codex", "codex", AgentKind::Codex),
    ];
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(tools)),
        // Only `claude` is on this machine; `codex` is not.
        Arc::new(FakeVersionProbe::new(&["claude"])),
        Arc::new(MockClock::new()),
    );

    let detected = agents.detect_installed().await.expect("detect");

    assert_eq!(detection(&detected, "Claude"), Detection::Installed);
    assert_eq!(detection(&detected, "Codex"), Detection::Missing);
}

#[tokio::test]
async fn tools_outside_the_probe_set_are_never_probed() {
    // A generic tool (user-configured) and a built-in type outside the documented probe set
    // (Copilot) are both skipped — even when their command would probe successfully, they are
    // never run, and report "not checked" rather than claiming the CLI is absent.
    let tools = vec![
        tool("My CLI", "mycli", AgentKind::Generic),
        tool("Copilot", "copilot", AgentKind::Copilot),
    ];
    let probe = Arc::new(FakeVersionProbe::new(&["mycli", "copilot"]));
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(tools)),
        probe.clone(),
        Arc::new(MockClock::new()),
    );

    let detected = agents.detect_installed().await.expect("detect");

    assert_eq!(
        detection(&detected, "My CLI"),
        Detection::Unknown,
        "a generic tool is never auto-detected, even if its command exists"
    );
    assert_eq!(
        detection(&detected, "Copilot"),
        Detection::Unknown,
        "a built-in type outside the probe set is never auto-detected"
    );
    assert_eq!(probe.probes(), 0, "neither tool reaches the probe at all");
}

#[tokio::test]
async fn a_probe_that_reaches_no_answer_is_not_reported_as_absent() {
    // The distinction the UI depends on: a probe that could not answer (no adapter wired, a
    // timeout in the real one) must stay separable from a CLI that genuinely is not installed,
    // so a broken probe cannot masquerade as "you don't have these tools".
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(vec![tool(
            "Claude",
            "claude",
            AgentKind::Claude,
        )])),
        Arc::new(NoopVersionProbe),
        Arc::new(MockClock::new()),
    );

    let detected = agents.detect_installed().await.expect("detect");

    assert_eq!(detection(&detected, "Claude"), Detection::Unknown);
}

#[tokio::test]
async fn detection_covers_every_configured_tool() {
    let tools = AgentTool::builtin_defaults();
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(tools.clone())),
        Arc::new(FakeVersionProbe::new(&[])),
        Arc::new(MockClock::new()),
    );

    let detected = agents.detect_installed().await.expect("detect");

    assert_eq!(
        detected.len(),
        tools.len(),
        "every configured tool is reported, present or not"
    );
}

#[tokio::test]
async fn detection_reports_tools_in_the_registry_order() {
    // The probes run concurrently, so the order results arrive in is whichever finished first;
    // the sweep must still read back in the registry's stable order.
    let tools = AgentTool::builtin_defaults();
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(tools.clone())),
        Arc::new(FakeVersionProbe::new(&[])),
        Arc::new(MockClock::new()),
    );

    let detected = agents.detect_installed().await.expect("detect");

    let order: Vec<_> = detected.iter().map(|d| d.tool.name.clone()).collect();
    let expected: Vec<_> = tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(order, expected);
}

#[tokio::test]
async fn detection_is_cached_within_the_ttl_and_refreshed_after_it() {
    let probe = Arc::new(FakeVersionProbe::new(&["claude"]));
    let clock = MockClock::new();
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(vec![tool(
            "Claude",
            "claude",
            AgentKind::Claude,
        )])),
        probe.clone(),
        Arc::new(clock.clone()),
    );

    agents.detect_installed().await.expect("first detect");
    agents.detect_installed().await.expect("second detect");
    // The second picker open within the window reused the first sweep: the CLI was probed once.
    assert_eq!(probe.probes(), 1);

    // Past the TTL the next open re-probes rather than serving the stale sweep.
    clock.advance(DETECT_CACHE_TTL + std::time::Duration::from_secs(1));
    agents.detect_installed().await.expect("third detect");
    assert_eq!(probe.probes(), 2);
}

#[tokio::test]
async fn an_explicit_redetect_re_probes_inside_the_cache_window() {
    // The user-facing "Detect" action. Within the TTL the cached sweep is what every other
    // reader gets; a deliberate re-check must still reach the CLIs, or a wrong cached answer
    // stays on screen until the TTL lapses with no way to correct it.
    let probe = Arc::new(FakeVersionProbe::new(&["claude"]));
    let clock = MockClock::new();
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(vec![tool(
            "Claude",
            "claude",
            AgentKind::Claude,
        )])),
        probe.clone(),
        Arc::new(clock.clone()),
    );

    agents.detect_installed().await.expect("first detect");
    assert_eq!(probe.probes(), 1);

    // No clock advance: still well inside the cache window.
    agents.redetect_installed().await.expect("redetect");
    assert_eq!(probe.probes(), 2, "an explicit redetect probes again");

    // And the refreshed sweep is what subsequent cached reads serve.
    agents.detect_installed().await.expect("cached read");
    assert_eq!(probe.probes(), 2, "the redetect repopulated the cache");
}
