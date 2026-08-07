//! Reading the NUL-separated log stream. The fixtures are the shape `git log -z --format=…` really
//! produces, captured from a real repository and edited only to stand in dummy names.

use super::*;

/// One record as the format prints it: every field NUL-separated, and `-z` making the record
/// separator a NUL as well.
fn record(id: &str, author: &str, at: &str, parents: &str, subject: &str) -> Vec<u8> {
    format!("{id}\0{author}\0{at}\0{parents}\0{subject}\0").into_bytes()
}

const ID: &str = "267f5bca317eb3c0d9f28cbbb9bd8631fa06295e";
const PARENT: &str = "565f350691";

#[test]
fn a_record_reads_back_as_the_commit_it_describes() {
    let output = record(
        ID,
        "Ada Lovelace",
        "1786052552",
        PARENT,
        "Record the negative result",
    );

    let commits = parse(&output);

    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].id, ID);
    assert_eq!(commits[0].author, "Ada Lovelace");
    assert_eq!(commits[0].authored_at, 1_786_052_552);
    assert_eq!(commits[0].subject, "Record the negative result");
    assert!(!commits[0].merge);
}

#[test]
fn a_commit_with_more_than_one_parent_is_a_merge() {
    // Counted, never read: a merge is a commit joining more than one line of history, whatever its
    // message says — and a subject beginning "Merge" is not itself evidence of one.
    let output = record(
        ID,
        "Ada Lovelace",
        "1786052552",
        "deabaa8268844d95 bcc397b0d1",
        "Merge pull request #137",
    );

    assert!(parse(&output)[0].merge);
}

#[test]
fn several_records_read_back_in_the_order_they_arrived() {
    let mut output = record(ID, "Ada Lovelace", "1786052552", PARENT, "The newest");
    output.extend(record(
        PARENT,
        "Grace Hopper",
        "1786049897",
        "ab30fb7",
        "The one before",
    ));

    let subjects: Vec<String> = parse(&output).into_iter().map(|c| c.subject).collect();

    assert_eq!(subjects, vec!["The newest", "The one before"]);
}

#[test]
fn a_subject_carrying_the_characters_a_person_writes_survives_intact() {
    // The two fields a person wrote may hold anything but a NUL, which is exactly why the format
    // separates on one: a tab, a quote or an equals sign in a subject is ordinary.
    let subject = "Answer the whole \"lowlight\" surface\tand a=b, per §4";
    let output = record(ID, "Ada O'Brien", "1786052552", PARENT, subject);

    let commits = parse(&output);

    assert_eq!(commits[0].subject, subject);
    assert_eq!(commits[0].author, "Ada O'Brien");
}

#[test]
fn the_first_commit_in_a_repository_has_no_parents_and_is_not_a_merge() {
    let output = record(ID, "Ada Lovelace", "1786052552", "", "Begin");

    assert!(!parse(&output)[0].merge);
}

#[test]
fn an_empty_history_reads_back_as_no_commits() {
    assert_eq!(parse(b""), Vec::new());
}

#[test]
fn a_record_whose_date_is_not_a_number_costs_that_record_and_no_other() {
    let mut output = record(ID, "Ada Lovelace", "not-a-date", PARENT, "Unreadable");
    output.extend(record(
        PARENT,
        "Grace Hopper",
        "1786049897",
        "ab30fb7",
        "Readable",
    ));

    let subjects: Vec<String> = parse(&output).into_iter().map(|c| c.subject).collect();

    assert_eq!(subjects, vec!["Readable"]);
}
