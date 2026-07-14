use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::testing::{FakeFeedbackRepo, MockClock};

fn feedback() -> (Feedback, Arc<FakeFeedbackRepo>, Arc<MockClock>) {
    let repo = Arc::new(FakeFeedbackRepo::new());
    let clock = Arc::new(MockClock::new());
    (Feedback::new(repo.clone(), clock.clone()), repo, clock)
}

#[test]
fn a_submission_is_trimmed_stamped_and_stored() {
    let (feedback, _repo, clock) = feedback();
    clock.advance(Duration::from_millis(1_500));

    let entry = feedback.submit("  the sidebar flickers  ").expect("submit");

    assert_eq!(entry.message, "the sidebar flickers");
    assert_eq!(entry.submitted_unix_millis, clock.now_unix_millis());
    let stored = feedback.list().expect("list");
    assert_eq!(stored, vec![entry]);
}

#[test]
fn entries_list_oldest_first_with_distinct_ids() {
    let (feedback, _repo, _clock) = feedback();

    let first = feedback.submit("first").expect("submit first");
    let second = feedback.submit("second").expect("submit second");

    assert_ne!(first.id, second.id);
    assert_eq!(feedback.list().expect("list"), vec![first, second]);
}

#[test]
fn a_blank_message_is_refused_before_it_persists() {
    let (feedback, repo, _clock) = feedback();

    assert!(matches!(
        feedback.submit("   \n  "),
        Err(FeedbackError::Empty)
    ));
    assert!(repo.list().expect("list").is_empty());
}

#[test]
fn an_oversized_message_is_refused_before_it_persists() {
    let (feedback, repo, _clock) = feedback();
    let oversized = "x".repeat(MAX_FEEDBACK_LEN + 1);

    assert!(matches!(
        feedback.submit(&oversized),
        Err(FeedbackError::TooLong)
    ));
    assert!(repo.list().expect("list").is_empty());
}

#[test]
fn a_message_at_exactly_the_limit_is_accepted() {
    let (feedback, _repo, _clock) = feedback();
    let at_limit = "x".repeat(MAX_FEEDBACK_LEN);

    let entry = feedback
        .submit(&at_limit)
        .expect("exactly the limit is allowed");
    assert_eq!(entry.message.chars().count(), MAX_FEEDBACK_LEN);
}

#[test]
fn the_limit_counts_characters_not_bytes() {
    let (feedback, _repo, _clock) = feedback();
    // MAX_FEEDBACK_LEN four-byte characters: at the character limit, but 4x over it in bytes. A
    // byte-based check would wrongly reject this as TooLong.
    let multibyte = "😀".repeat(MAX_FEEDBACK_LEN);
    assert!(
        multibyte.len() > MAX_FEEDBACK_LEN,
        "the fixture is multibyte"
    );

    let entry = feedback
        .submit(&multibyte)
        .expect("the limit is measured in characters, so this is accepted");
    assert_eq!(entry.message.chars().count(), MAX_FEEDBACK_LEN);
}

#[test]
fn a_full_store_refuses_further_submissions() {
    let (feedback, repo, _clock) = feedback();
    for n in 0..MAX_FEEDBACK_ENTRIES {
        feedback
            .submit(&format!("note {n}"))
            .expect("fill the store to its cap");
    }

    assert!(matches!(
        feedback.submit("one more"),
        Err(FeedbackError::Full)
    ));
    assert_eq!(repo.count().expect("count"), MAX_FEEDBACK_ENTRIES);
}
