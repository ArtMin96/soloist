//! Tests for the idle tracker — that it classifies tracked agents, reports the provider each was
//! launched under, ignores unknown ones, and prunes agents that have left the registry.

use std::collections::HashSet;

use super::{AgentActivity, AgentKind, IdleTracker, ProcessId, TerminalActivity};

fn output(output_seq: u64) -> TerminalActivity {
    TerminalActivity {
        output_seq,
        title: None,
        tail: Vec::new(),
    }
}

#[test]
fn observe_classifies_a_tracked_agent() {
    let tracker = IdleTracker::new();
    let id = ProcessId::next();
    tracker.track(id, AgentKind::Claude);
    assert_eq!(
        tracker.observe(id, &output(20)),
        Some(AgentActivity::Working)
    );
}

#[test]
fn observe_is_a_noop_for_an_untracked_id() {
    let tracker = IdleTracker::new();
    assert_eq!(tracker.observe(ProcessId::next(), &output(20)), None);
}

#[test]
fn provider_reports_the_kind_an_agent_was_tracked_under() {
    let tracker = IdleTracker::new();
    let id = ProcessId::next();
    tracker.track(id, AgentKind::Gemini);

    assert_eq!(tracker.provider(id), Some(AgentKind::Gemini));

    // Re-tracking replaces the entry, so the provider follows the latest launch rather than
    // lingering from the previous one.
    tracker.track(id, AgentKind::Codex);
    assert_eq!(tracker.provider(id), Some(AgentKind::Codex));
}

#[test]
fn an_untracked_agent_has_no_provider() {
    let tracker = IdleTracker::new();
    let never_tracked = ProcessId::next();
    let pruned = ProcessId::next();
    tracker.track(pruned, AgentKind::Gemini);

    tracker.retain_live(&HashSet::new());

    assert_eq!(tracker.provider(never_tracked), None);
    assert_eq!(
        tracker.provider(pruned),
        None,
        "an agent that left the registry no longer reports a provider"
    );
}

#[test]
fn activity_snapshot_reports_only_classified_agents() {
    // The snapshot seeds the UI's idle badges: a classified agent appears with its current
    // activity; a tracked-but-never-observed agent (still starting up) has no activity yet and is
    // omitted, so the seed never invents a badge the core has not classified.
    let tracker = IdleTracker::new();
    let observed = ProcessId::next();
    let never_observed = ProcessId::next();
    tracker.track(observed, AgentKind::Claude);
    tracker.track(never_observed, AgentKind::Claude);
    tracker.observe(observed, &output(20));

    assert_eq!(
        tracker.activity_snapshot(),
        vec![(observed, AgentActivity::Working)]
    );
}

#[test]
fn retain_live_drops_departed_agents() {
    let tracker = IdleTracker::new();
    let kept = ProcessId::next();
    let gone = ProcessId::next();
    tracker.track(kept, AgentKind::Claude);
    tracker.track(gone, AgentKind::Claude);

    tracker.retain_live(&HashSet::from([kept]));

    assert_eq!(tracker.tracked(), vec![kept]);
    assert_eq!(
        tracker.observe(gone, &output(20)),
        None,
        "a pruned agent is no longer classified"
    );
}
