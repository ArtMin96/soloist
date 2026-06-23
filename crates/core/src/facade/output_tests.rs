use super::*;

/// A list whose total size exceeds the reply budget: each line is ~100 KiB with a unique
/// prefix, so a handful already overflows the 1 MiB cap and forces a trim while staying
/// distinguishable by `first`/`last`.
fn oversized_lines() -> Vec<String> {
    (0..20)
        .map(|n| format!("line {n} ") + &"x".repeat(100 * 1024))
        .collect()
}

#[test]
fn within_budget_keeps_the_newest_lines_under_the_cap() {
    let lines = oversized_lines();
    let kept = within_reply_budget(lines.clone(), Keep::Newest);

    let bytes: usize = kept.iter().map(|line| line.len() + 1).sum();
    assert!(
        bytes <= MAX_REPLY_BYTES,
        "the kept reply fits the byte budget"
    );
    assert!(kept.len() < lines.len(), "an oversized reply is trimmed");
    assert_eq!(
        kept.last(),
        lines.last(),
        "a tail keeps the most recent line"
    );
}

#[test]
fn within_budget_keeps_the_earliest_matches_under_the_cap() {
    let lines = oversized_lines();
    let kept = within_reply_budget(lines.clone(), Keep::Earliest);

    let bytes: usize = kept.iter().map(|line| line.len() + 1).sum();
    assert!(
        bytes <= MAX_REPLY_BYTES,
        "the kept reply fits the byte budget"
    );
    assert!(kept.len() < lines.len(), "an oversized reply is trimmed");
    assert_eq!(
        kept.first(),
        lines.first(),
        "an ordered match list keeps its earliest entries"
    );
}

#[test]
fn within_budget_returns_everything_under_the_cap_unchanged() {
    let lines = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    assert_eq!(within_reply_budget(lines.clone(), Keep::Newest), lines);
    assert_eq!(within_reply_budget(lines.clone(), Keep::Earliest), lines);
}

#[test]
fn within_budget_of_nothing_is_nothing() {
    assert!(within_reply_budget(Vec::new(), Keep::Newest).is_empty());
}
