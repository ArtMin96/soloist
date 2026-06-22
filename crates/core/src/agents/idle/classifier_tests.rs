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
