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

fn installed(detected: &[DetectedTool], name: &str) -> bool {
    detected
        .iter()
        .find(|d| d.tool.name == name)
        .map(|d| d.installed)
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

    assert!(installed(&detected, "Claude"), "claude is present");
    assert!(!installed(&detected, "Codex"), "codex is absent");
}

#[tokio::test]
async fn tools_outside_the_probe_set_are_never_probed() {
    // A generic tool (user-configured) and a built-in type outside the documented probe set
    // (Copilot) are both skipped — even when their command would probe successfully, they
    // report absent rather than being run.
    let tools = vec![
        tool("My CLI", "mycli", AgentKind::Generic),
        tool("Copilot", "copilot", AgentKind::Copilot),
    ];
    let agents = Agents::new(
        Arc::new(FakeAgentToolRepo::new(tools)),
        Arc::new(FakeVersionProbe::new(&["mycli", "copilot"])),
        Arc::new(MockClock::new()),
    );

    let detected = agents.detect_installed().await.expect("detect");

    assert!(
        !installed(&detected, "My CLI"),
        "a generic tool is never auto-detected, even if its command exists"
    );
    assert!(
        !installed(&detected, "Copilot"),
        "a built-in type outside the probe set is never auto-detected"
    );
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
