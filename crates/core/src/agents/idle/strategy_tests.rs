//! Fixture tests for the per-provider idle heuristics — pure, no clock or PTY. They feed a
//! strategy recorded terminal signals and assert the activity it derives, which is where the
//! "quiet ≠ done" correctness of idle detection is pinned down.

use super::{
    strategy_for, AgentActivity, AgentKind, AgentMemory, TerminalActivity,
    IDLE_AFTER_QUIET_SAMPLES,
};

/// Builds a terminal-signals snapshot from its parts.
fn signals(output_seq: u64, title: Option<&str>, tail: &[&str]) -> TerminalActivity {
    TerminalActivity {
        output_seq,
        title: title.map(str::to_string),
        tail: tail.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn output_delta_reports_working_while_output_flows() {
    let strategy = strategy_for(AgentKind::Claude);
    let mut memory = AgentMemory::default();
    assert_eq!(
        strategy.classify(&mut memory, &signals(10, None, &[]), AgentActivity::Idle),
        AgentActivity::Working
    );
    assert_eq!(
        strategy.classify(&mut memory, &signals(25, None, &[]), AgentActivity::Working),
        AgentActivity::Working
    );
}

#[test]
fn output_delta_goes_idle_after_a_quiet_window() {
    let strategy = strategy_for(AgentKind::Claude);
    let mut memory = AgentMemory::default();
    let mut activity = strategy.classify(&mut memory, &signals(10, None, &[]), AgentActivity::Idle);
    assert_eq!(activity, AgentActivity::Working);
    // A brief pause holds Working (settling)...
    for _ in 0..IDLE_AFTER_QUIET_SAMPLES - 1 {
        activity = strategy.classify(&mut memory, &signals(10, None, &[]), activity);
        assert_eq!(activity, AgentActivity::Working, "still settling, not yet idle");
    }
    // ...then idle once quiet long enough.
    activity = strategy.classify(&mut memory, &signals(10, None, &[]), activity);
    assert_eq!(activity, AgentActivity::Idle);
}

#[test]
fn output_delta_reports_permission_on_a_prompt_even_when_quiet() {
    let strategy = strategy_for(AgentKind::Claude);
    let mut memory = AgentMemory::default();
    // Output is quiet (same seq) but a prompt sits at the tail: Permission, not Idle — the
    // distinction the whole five-state design exists to make.
    let observed = signals(10, None, &["Do you want to proceed? (y/n)"]);
    assert_eq!(
        strategy.classify(&mut memory, &observed, AgentActivity::Idle),
        AgentActivity::Permission
    );
}

#[test]
fn output_delta_agent_that_never_outputs_stays_idle() {
    let strategy = strategy_for(AgentKind::OpenCode);
    let mut memory = AgentMemory::default();
    let mut activity = AgentActivity::Idle;
    for _ in 0..5 {
        activity = strategy.classify(&mut memory, &signals(0, None, &[]), activity);
    }
    assert_eq!(activity, AgentActivity::Idle);
}

#[test]
fn title_stability_works_while_the_title_changes_then_idles_when_stable() {
    let strategy = strategy_for(AgentKind::Codex);
    let mut memory = AgentMemory::default();
    assert_eq!(
        strategy.classify(
            &mut memory,
            &signals(0, Some("building 1/3"), &[]),
            AgentActivity::Idle
        ),
        AgentActivity::Working
    );
    assert_eq!(
        strategy.classify(
            &mut memory,
            &signals(0, Some("building 2/3"), &[]),
            AgentActivity::Working
        ),
        AgentActivity::Working
    );
    let mut activity = AgentActivity::Working;
    for _ in 0..IDLE_AFTER_QUIET_SAMPLES {
        activity = strategy.classify(&mut memory, &signals(0, Some("building 2/3"), &[]), activity);
    }
    assert_eq!(activity, AgentActivity::Idle);
}

#[test]
fn title_status_maps_status_keywords_to_activities() {
    let strategy = strategy_for(AgentKind::Gemini);
    let mut memory = AgentMemory::default();
    assert_eq!(
        strategy.classify(
            &mut memory,
            &signals(0, Some("Gemini is thinking…"), &[]),
            AgentActivity::Idle
        ),
        AgentActivity::Thinking
    );
    assert_eq!(
        strategy.classify(
            &mut memory,
            &signals(0, Some("Running tool"), &[]),
            AgentActivity::Thinking
        ),
        AgentActivity::Working
    );
    assert_eq!(
        strategy.classify(
            &mut memory,
            &signals(0, Some("Error: quota exceeded"), &[]),
            AgentActivity::Working
        ),
        AgentActivity::Error
    );
}

#[test]
fn title_status_with_no_keyword_falls_back_to_idle_when_quiet() {
    let strategy = strategy_for(AgentKind::Gemini);
    let mut memory = AgentMemory::default();
    let mut activity = AgentActivity::Working;
    for _ in 0..IDLE_AFTER_QUIET_SAMPLES + 1 {
        activity = strategy.classify(&mut memory, &signals(0, Some("gemini"), &[]), activity);
    }
    assert_eq!(activity, AgentActivity::Idle);
}
