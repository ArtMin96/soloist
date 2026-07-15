//! Tests for the idle tracker — that it classifies tracked agents, ignores unknown ones, and
//! prunes agents that have left the registry.

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
