//! Unit tests for reading a branch listing. What the listing *does* against a real repository is
//! `tests/branch.rs`; this covers the reading of the format the adapter asked for.

use super::parse;

/// One record as `for-each-ref` produces it under the format this adapter asks for: the name, the
/// upstream, and the marker for the branch that is checked out — the last of which is a space when
/// the branch is not.
fn record(name: &str, upstream: &str, head: &str) -> String {
    format!("{name}\0{upstream}\0{head}\n")
}

#[test]
fn a_branch_is_read_with_its_upstream_and_whether_it_is_the_one_checked_out() {
    let listed = format!(
        "{}{}",
        record("main", "origin/main", "*"),
        record("feature", "", " ")
    );

    let branches = parse(listed.as_bytes());

    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].name, "main");
    assert_eq!(branches[0].upstream.as_deref(), Some("origin/main"));
    assert!(branches[0].head);
    assert_eq!(branches[1].name, "feature");
    assert_eq!(
        branches[1].upstream, None,
        "an empty upstream field means the branch tracks nothing, not that it tracks \"\"",
    );
    assert!(!branches[1].head);
}

#[test]
fn a_name_holding_a_space_survives_being_read() {
    // Version control allows a space in a ref name, so the fields cannot be split on one.
    let branches = parse(record("wip/two words", "", " ").as_bytes());

    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "wip/two words");
}

#[test]
fn a_record_missing_a_field_is_left_out_rather_than_guessed_at() {
    let listed = format!("half-a-record\n{}", record("main", "", "*"));

    let branches = parse(listed.as_bytes());

    assert_eq!(
        branches.len(),
        1,
        "a listing is a set of branches to offer, so one nobody could act on is better unoffered",
    );
    assert_eq!(branches[0].name, "main");
}
