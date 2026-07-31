//! Tests for the edge-triggered classifier wrapper — that it emits on the first sample and
//! on every change, holds silent otherwise, and re-emits after a reset.

use super::{AgentActivity, AgentKind, Classifier, TerminalActivity};

fn signals(output_seq: u64, title: Option<&str>, tail: &[&str]) -> TerminalActivity {
    TerminalActivity {
        output_seq,
        title: title.map(str::to_string),
        tail: tail.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn the_first_sample_always_emits() {
    let mut classifier = Classifier::new(AgentKind::Claude);
    assert_eq!(
        classifier.observe(&signals(5, None, &[])),
        Some(AgentActivity::Working)
    );
}

#[test]
fn a_quiet_agent_first_emits_idle() {
    let mut classifier = Classifier::new(AgentKind::Claude);
    assert_eq!(
        classifier.observe(&signals(0, None, &[])),
        Some(AgentActivity::Idle)
    );
}

#[test]
fn quiet_before_an_agent_has_worked_is_not_observed_as_a_finished_turn() {
    // An agent CLI outputs nothing while it starts up, so the heuristic reads quiet and reports
    // Idle — which is what the terminal shows, and what a badge should say. It is not a finished
    // turn, though: a caller waiting for this agent to *finish* must not be answered by the quiet
    // of one that has not begun, so nothing is observed until the agent is first seen working.
    let mut classifier = Classifier::new(AgentKind::Claude);
    for _ in 0..4 {
        classifier.observe(&signals(0, None, &[]));
    }
    assert_eq!(classifier.current(), Some(AgentActivity::Idle));
    assert_eq!(
        classifier.observed().latest(),
        None,
        "an agent that has only ever been quiet has not begun a turn"
    );

    // Output arrives: the agent is working, and from here its quiet means something.
    classifier.observe(&signals(64, None, &[]));
    assert_eq!(classifier.observed().latest(), Some(AgentActivity::Working));
    for _ in 0..3 {
        classifier.observe(&signals(64, None, &[]));
    }
    assert_eq!(
        classifier.observed().latest(),
        Some(AgentActivity::Idle),
        "quiet after a turn is a finished turn"
    );
}

#[test]
fn reset_drops_the_turn_observed_before_the_agent_stopped() {
    // A stopped agent's turn is over; the next run's turn is its own. Keeping the old one would
    // let a relaunched agent's start-up quiet answer for work it did before it stopped.
    let mut classifier = Classifier::new(AgentKind::Claude);
    classifier.observe(&signals(64, None, &[]));
    for _ in 0..3 {
        classifier.observe(&signals(64, None, &[]));
    }
    assert_eq!(classifier.observed().latest(), Some(AgentActivity::Idle));

    classifier.reset();
    assert_eq!(classifier.observed().latest(), None);
}

#[test]
fn a_launched_agent_stays_recorded_as_launched_once_it_stops() {
    // A process at rest is at rest either because it ran and ended or because nobody ever started
    // it, and the two share a status. Only the launch tells them apart, so stopping an agent must
    // forget the turn it was in without forgetting that it ran at all.
    let mut classifier = Classifier::new(AgentKind::Claude);
    assert!(
        classifier.observed().has_launched(),
        "a classifier exists only because its agent was launched"
    );
    classifier.observe(&signals(64, None, &[]));

    classifier.reset();
    assert!(
        classifier.observed().has_launched(),
        "stopping an agent does not unlaunch it"
    );
}

#[test]
fn an_unchanged_activity_does_not_re_emit() {
    let mut classifier = Classifier::new(AgentKind::Claude);
    assert_eq!(
        classifier.observe(&signals(5, None, &[])),
        Some(AgentActivity::Working)
    );
    // Still producing output: still Working, so no edge and no event.
    assert_eq!(classifier.observe(&signals(9, None, &[])), None);
}

#[test]
fn reset_makes_the_next_sample_emit_again() {
    let mut classifier = Classifier::new(AgentKind::Claude);
    assert_eq!(
        classifier.observe(&signals(5, None, &[])),
        Some(AgentActivity::Working)
    );
    classifier.reset();
    assert_eq!(
        classifier.observe(&signals(10, None, &[])),
        Some(AgentActivity::Working)
    );
}
