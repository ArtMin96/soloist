use super::*;

#[test]
fn a_suggestion_shows_up_to_its_budget_then_decays() {
    let suggestions = Suggestions::default();
    // The first SHOW_BUDGET takes return the hint...
    for _ in 0..SHOW_BUDGET {
        assert!(suggestions.take("spawn_agent").is_some());
    }
    // ...and every take after that is silent.
    assert!(suggestions.take("spawn_agent").is_none());
    assert!(suggestions.take("spawn_agent").is_none());
}

#[test]
fn a_tool_without_a_suggestion_never_shows_one() {
    let suggestions = Suggestions::default();
    assert!(suggestions.take("whoami").is_none());
    assert!(suggestions.take("list_processes").is_none());
}

#[test]
fn tools_sharing_a_hint_share_its_decay_budget() {
    // start_process and restart_process point to the same hint, so together they exhaust one budget
    // rather than two — the caller sees the "don't poll for readiness" nudge a bounded number of
    // times regardless of which of the two it uses.
    let suggestions = Suggestions::default();
    assert_eq!(hint_for("start_process"), hint_for("restart_process"));
    assert!(suggestions.take("start_process").is_some());
    assert!(suggestions.take("restart_process").is_some());
    assert!(suggestions.take("start_process").is_none());
}

#[test]
fn distinct_suggestions_decay_independently() {
    let suggestions = Suggestions::default();
    // Exhausting one tool's suggestion does not silence another's.
    for _ in 0..SHOW_BUDGET {
        assert!(suggestions.take("spawn_agent").is_some());
    }
    assert!(suggestions.take("spawn_agent").is_none());
    assert!(suggestions.take("todo_create").is_some());
}
