//! Tests for the conservative permission-prompt heuristic.

use super::looks_like_permission_prompt;

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_yes_no_prompt_at_the_tail_is_a_permission() {
    assert!(looks_like_permission_prompt(&lines(&[
        "Editing src/main.rs",
        "Do you want to proceed? (y/n)",
    ])));
}

#[test]
fn an_approval_question_is_a_permission() {
    assert!(looks_like_permission_prompt(&lines(&[
        "Run `rm -rf build`?",
        "Allow this action?",
    ])));
}

#[test]
fn ordinary_output_is_not_a_permission() {
    assert!(!looks_like_permission_prompt(&lines(&[
        "Compiling soloist v0.1.0",
        "Finished in 3.2s",
    ])));
}

#[test]
fn a_permission_denied_error_is_not_mistaken_for_a_prompt() {
    // The bare word "permission" is intentionally not a cue, so a failure line does not
    // read as the agent asking for approval.
    assert!(!looks_like_permission_prompt(&lines(&[
        "error: permission denied (os error 13)",
    ])));
}

#[test]
fn a_cue_buried_far_above_the_tail_is_ignored() {
    // Only the last few non-empty lines are scanned, so an old prompt that has scrolled
    // up — with fresh output below it — does not keep reporting a permission.
    let mut text = vec!["Continue? (y/n)".to_string()];
    for n in 0..10 {
        text.push(format!("line {n}"));
    }
    assert!(!looks_like_permission_prompt(&text));
}
