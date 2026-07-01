use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast::error::TryRecvError;

use crate::agents::{AgentActivity, AgentKind, AgentTool, PromptMode};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::settings::{AgentSettings, Settings, SettingsRepo};
use crate::testing::{
    CannedSummaryRunner, FailingSummaryRunner, FakeAgentToolRepo, FakeOutputSnapshot,
    FakeSettingsRepo, MockClock,
};

use super::{clamp, plan_summary, SummaryReactor, COOLDOWN};

/// The built-in Claude tool — a provider with a documented headless one-shot.
fn claude_tool() -> AgentTool {
    AgentTool {
        name: "Claude".into(),
        command: "claude".into(),
        default_args: Vec::new(),
        kind: AgentKind::Claude,
        prompt_mode: PromptMode::AppendedArg,
    }
}

/// A settings repo seeded with the summarization opt-in (`tool` off = summarization off).
fn settings_repo(tool: Option<&str>, model: Option<&str>) -> Arc<FakeSettingsRepo<(), Settings>> {
    let repo = FakeSettingsRepo::new();
    let settings = Settings {
        agents: AgentSettings {
            summarizer_tool: tool.map(str::to_string),
            summarizer_model: model.map(str::to_string),
        },
        ..Default::default()
    };
    repo.save(&(), &settings).expect("seed settings");
    Arc::new(repo)
}

// ── The pure decision (`plan_summary`): the whole off/missing/unsupported/empty/happy matrix ──

#[test]
fn plan_is_none_when_summarization_is_off() {
    let agents = AgentSettings::default();
    assert!(plan_summary(&agents, &[claude_tool()], &["cargo build".to_string()]).is_none());
}

#[test]
fn plan_is_none_when_the_configured_tool_is_not_registered() {
    let agents = AgentSettings {
        summarizer_tool: Some("Ghost".into()),
        summarizer_model: None,
    };
    assert!(plan_summary(&agents, &[claude_tool()], &["cargo build".to_string()]).is_none());
}

#[test]
fn plan_is_none_for_a_provider_without_a_headless_one_shot() {
    let amp = AgentTool {
        name: "Amp".into(),
        command: "amp".into(),
        default_args: Vec::new(),
        kind: AgentKind::Amp,
        prompt_mode: PromptMode::AppendedArg,
    };
    let agents = AgentSettings {
        summarizer_tool: Some("Amp".into()),
        summarizer_model: None,
    };
    assert!(plan_summary(&agents, &[amp], &["cargo build".to_string()]).is_none());
}

#[test]
fn plan_is_none_when_there_is_no_output_to_summarize() {
    let agents = AgentSettings {
        summarizer_tool: Some("Claude".into()),
        summarizer_model: Some("sonnet".into()),
    };
    assert!(plan_summary(&agents, &[claude_tool()], &[]).is_none());
}

#[test]
fn plan_composes_the_configured_tools_invocation() {
    let agents = AgentSettings {
        summarizer_tool: Some("Claude".into()),
        summarizer_model: Some("sonnet".into()),
    };
    let invocation = plan_summary(&agents, &[claude_tool()], &["cargo build".to_string()])
        .expect("an invocation for a supported, enabled provider");
    assert!(invocation
        .command_line
        .starts_with("claude --model sonnet -p "));
    assert_eq!(invocation.stdin, None);
}

#[test]
fn clamp_keeps_short_text_and_truncates_long_text_on_char_boundaries() {
    assert_eq!(clamp("done", 200), "done");
    assert_eq!(clamp("abcdef", 3), "abc");
    // A multi-byte char is never split: three chars are h, é, l.
    assert_eq!(clamp("héllo", 3), "hél");
}

// ── The reactor: publishing, degradation, and cadence (deterministic, via the private on_idle) ──

#[tokio::test]
async fn on_idle_publishes_a_summary_when_enabled() {
    let bus = EventBus::new(64);
    let runner = Arc::new(CannedSummaryRunner::new("Building the project"));
    let mut reactor = SummaryReactor::new(
        Arc::new(MockClock::new()),
        runner.clone(),
        settings_repo(Some("Claude"), Some("sonnet")),
        Arc::new(FakeAgentToolRepo::new(vec![claude_tool()])),
        Arc::new(FakeOutputSnapshot::new(vec!["cargo build".into()])),
        bus.clone(),
    );
    let mut rx = bus.subscribe();
    let id = ProcessId::next();

    reactor.on_idle(id).await;

    match rx.try_recv() {
        Ok(DomainEvent::AgentSummary { id: got, text }) => {
            assert_eq!(got, id);
            assert_eq!(text, "Building the project");
        }
        other => panic!("expected an AgentSummary, got {other:?}"),
    }
    let invocations = runner.invocations();
    assert_eq!(invocations.len(), 1);
    assert!(invocations[0]
        .command_line
        .starts_with("claude --model sonnet -p "));
}

#[tokio::test]
async fn on_idle_is_silent_when_the_summarizer_fails() {
    let bus = EventBus::new(64);
    let mut reactor = SummaryReactor::new(
        Arc::new(MockClock::new()),
        Arc::new(FailingSummaryRunner),
        settings_repo(Some("Claude"), Some("sonnet")),
        Arc::new(FakeAgentToolRepo::new(vec![claude_tool()])),
        Arc::new(FakeOutputSnapshot::new(vec!["cargo build".into()])),
        bus.clone(),
    );
    let mut rx = bus.subscribe();

    reactor.on_idle(ProcessId::next()).await;

    // A failing summarizer degrades to nothing — no event, and (the test completing) no panic.
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test]
async fn on_idle_rate_limits_repeat_summaries_per_agent() {
    let bus = EventBus::new(64);
    let clock = MockClock::new();
    let runner = Arc::new(CannedSummaryRunner::new("working"));
    let mut reactor = SummaryReactor::new(
        Arc::new(clock.clone()),
        runner.clone(),
        settings_repo(Some("Claude"), Some("sonnet")),
        Arc::new(FakeAgentToolRepo::new(vec![claude_tool()])),
        Arc::new(FakeOutputSnapshot::new(vec!["cargo build".into()])),
        bus.clone(),
    );
    let mut rx = bus.subscribe();
    let id = ProcessId::next();

    reactor.on_idle(id).await;
    assert!(matches!(
        rx.try_recv(),
        Ok(DomainEvent::AgentSummary { .. })
    ));

    reactor.on_idle(id).await; // within the cooldown → suppressed
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));

    clock.advance(COOLDOWN + Duration::from_secs(1));
    reactor.on_idle(id).await; // cooldown elapsed → summarizes again
    assert!(matches!(
        rx.try_recv(),
        Ok(DomainEvent::AgentSummary { .. })
    ));

    assert_eq!(runner.invocations().len(), 2);
}

#[tokio::test]
async fn run_summarizes_an_agent_that_goes_idle() {
    let bus = EventBus::new(64);
    let runner = Arc::new(CannedSummaryRunner::new("Reviewing the diff"));
    let reactor = SummaryReactor::new(
        Arc::new(MockClock::new()),
        runner,
        settings_repo(Some("Claude"), Some("sonnet")),
        Arc::new(FakeAgentToolRepo::new(vec![claude_tool()])),
        Arc::new(FakeOutputSnapshot::new(vec!["git diff".into()])),
        bus.clone(),
    );
    let mut rx = bus.subscribe();
    let id = ProcessId::next();

    tokio::spawn(reactor.run());
    bus.publish(DomainEvent::AgentActivityChanged {
        id,
        state: AgentActivity::Idle,
    });

    let summary = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(DomainEvent::AgentSummary { id: got, text }) = rx.recv().await {
                return (got, text);
            }
        }
    })
    .await
    .expect("a summary is published within the timeout");
    assert_eq!(summary.0, id);
    assert_eq!(summary.1, "Reviewing the diff");
}
