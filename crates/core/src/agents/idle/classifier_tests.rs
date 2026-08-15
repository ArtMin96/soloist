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
fn an_agent_without_provider_evidence_is_not_classified() {
    let mut classifier = Classifier::new(AgentKind::Claude);
    assert_eq!(classifier.observe(&signals(0, None, &[])), None);
    assert_eq!(classifier.current(), None);
}

#[test]
fn title_provider_without_a_title_is_not_classified() {
    let mut classifier = Classifier::new(AgentKind::Codex);
    assert_eq!(classifier.observe(&signals(9, None, &["starting"])), None);
    assert_eq!(classifier.current(), None);
}

#[test]
fn an_agent_that_never_outputs_is_never_classified() {
    // Silence is not availability: an agent that produces its provider's signal at no point in
    // the run stays unclassified for the whole run rather than settling to Idle, so nothing
    // gated on idle — a queued briefing, an addressed task, a fire-when-idle quorum — is woken
    // by an agent Soloist has heard nothing from.
    let mut classifier = Classifier::new(AgentKind::OpenCode);
    for _ in 0..5 {
        assert_eq!(classifier.observe(&signals(0, None, &[])), None);
    }
    assert_eq!(classifier.current(), None);
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
