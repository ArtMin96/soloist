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
    assert_eq!(tracker.observe(id, &output(20)), Some(AgentActivity::Working));
}

#[test]
fn observe_is_a_noop_for_an_untracked_id() {
    let tracker = IdleTracker::new();
    assert_eq!(tracker.observe(ProcessId::next(), &output(20)), None);
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
