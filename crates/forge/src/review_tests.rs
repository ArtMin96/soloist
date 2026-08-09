//! What the tool reports about an open pull request, read into the core's vocabulary.
//!
//! Every payload here is the real tool's shape, captured from `gh 2.92.0` against public
//! repositories and scrubbed — including the two kinds of check entry, which differ in the name of
//! every field they carry.

use soloist_core::CheckState;

use super::{ReviewPayload, RollupEntry};

/// One rollup entry read as a check, or `None` for a kind this does not know.
fn check(payload: &str) -> Option<soloist_core::CheckRun> {
    serde_json::from_str::<RollupEntry>(payload)
        .expect("the entry parses")
        .check()
}

/// Where an entry reports it stands.
fn state(payload: &str) -> CheckState {
    check(payload).expect("a known kind").state
}

#[test]
fn a_check_that_has_not_finished_is_still_pending_whatever_it_has_concluded_so_far() {
    for status in ["QUEUED", "IN_PROGRESS", "WAITING", "REQUESTED", "PENDING"] {
        let entry = format!(
            r#"{{"__typename":"CheckRun","name":"build","status":"{status}","conclusion":null,"workflowName":"Tests","detailsUrl":null}}"#
        );
        assert_eq!(state(&entry), CheckState::Pending, "status {status}");
    }
    // The status is what says whether a conclusion means anything at all. A check that is running
    // again after failing would otherwise be reported by the verdict it is in the middle of
    // replacing — which is the one reading that would send somebody to fix a check that is fixing
    // itself.
    assert_eq!(
        state(
            r#"{"__typename":"CheckRun","name":"build","status":"IN_PROGRESS","conclusion":"FAILURE","workflowName":"","detailsUrl":null}"#
        ),
        CheckState::Pending,
    );
}

#[test]
fn what_a_finished_check_concluded_decides_what_a_reader_is_shown() {
    let concluded = |conclusion: &str| {
        state(&format!(
            r#"{{"__typename":"CheckRun","name":"build","status":"COMPLETED","conclusion":"{conclusion}","workflowName":"","detailsUrl":null}}"#
        ))
    };

    assert_eq!(concluded("SUCCESS"), CheckState::Passed);
    assert_eq!(
        concluded("NEUTRAL"),
        CheckState::Passed,
        "a check that declined to object did not object",
    );
    for objecting in ["FAILURE", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"] {
        assert_eq!(
            concluded(objecting),
            CheckState::Failed,
            "every way of not passing reads as failing to somebody deciding whether to merge: \
             {objecting}",
        );
    }
    assert_eq!(concluded("SKIPPED"), CheckState::Skipped);
    assert_eq!(
        concluded("STALE"),
        CheckState::Skipped,
        "an answer about commits that have been replaced judges nothing about what is proposed now",
    );
    assert_eq!(concluded("CANCELLED"), CheckState::Cancelled);
}

#[test]
fn a_conclusion_the_service_has_newly_learnt_to_say_does_not_take_the_other_checks_with_it() {
    let listed = r#"[{"number":12,"url":"https://forge.example/pull/12","title":"t","state":"OPEN","isDraft":false,"baseRefName":"main","headRefName":"feature","statusCheckRollup":[{"__typename":"CheckRun","name":"new","status":"COMPLETED","conclusion":"SOMETHING_NEW","workflowName":"","detailsUrl":null},{"__typename":"CheckRun","name":"build","status":"COMPLETED","conclusion":"FAILURE","workflowName":"","detailsUrl":null}],"reviews":[],"comments":[]}]"#;

    let review = crate::json::first_review(listed.as_bytes())
        .expect("the answer is readable")
        .expect("the branch has one open");

    assert_eq!(review.checks[0].state, CheckState::Unknown);
    assert_eq!(
        review.checks[1].state,
        CheckState::Failed,
        "one unrecognised word must not take the rest of the list off the surface",
    );
}

#[test]
fn the_older_kind_of_check_is_read_by_its_own_field_names() {
    let entry = r#"{"__typename":"StatusContext","context":"Vercel – ui","startedAt":"2026-08-09T08:12:03Z","state":"FAILURE","targetUrl":"https://vercel.example/job"}"#;

    let read = check(entry).expect("a known kind");

    assert_eq!(
        read.name, "Vercel – ui",
        "it names itself in a different field from the newer kind, so reading one as the other \
         loses the name",
    );
    assert_eq!(read.state, CheckState::Failed);
    assert_eq!(read.url.as_deref(), Some("https://vercel.example/job"));
    assert_eq!(read.workflow, None);
}

#[test]
fn the_older_kinds_smaller_vocabulary_is_mapped_word_for_word() {
    let reported = |state: &str| {
        self::state(&format!(
            r#"{{"__typename":"StatusContext","context":"Vercel","state":"{state}","targetUrl":null}}"#
        ))
    };

    assert_eq!(reported("SUCCESS"), CheckState::Passed);
    assert_eq!(reported("PENDING"), CheckState::Pending);
    assert_eq!(
        reported("EXPECTED"),
        CheckState::Pending,
        "a status somebody promised and has not sent is one nothing has concluded",
    );
    assert_eq!(reported("FAILURE"), CheckState::Failed);
    assert_eq!(
        reported("ERROR"),
        CheckState::Failed,
        "a system that broke reporting a status has not said the change is fine",
    );
    assert_eq!(reported("SOMETHING_NEW"), CheckState::Unknown);
}

#[test]
fn a_kind_of_check_this_does_not_know_is_left_out_rather_than_guessed_at() {
    assert!(
        check(r#"{"__typename":"SomethingElse","whatever":1}"#).is_none(),
        "a check whose name and state could only be guessed at is worse than no row",
    );
}

#[test]
fn a_workflow_the_service_left_blank_is_no_workflow_at_all() {
    let read = check(
        r#"{"__typename":"CheckRun","name":"Socket Security","status":"COMPLETED","conclusion":"SUCCESS","workflowName":"","detailsUrl":null}"#,
    )
    .expect("a known kind");

    assert_eq!(
        read.workflow, None,
        "an empty name would render as a workflow called nothing",
    );
}

#[test]
fn a_review_submitted_with_nothing_written_on_it_is_not_a_conversation() {
    let payload: ReviewPayload = serde_json::from_str(
        r#"{"statusCheckRollup":[],"reviews":[{"id":"PRR_1","author":{"login":"octocat"},"body":""},{"id":"PRR_2","author":{"login":"octocat"},"body":"looks wrong to me"}],"comments":[]}"#,
    )
    .expect("parses");

    let (_, threads) = payload.into_parts();

    assert_eq!(
        threads.len(),
        1,
        "an empty review says nothing a reader needs"
    );
    assert_eq!(threads[0].comments[0].body, "looks wrong to me");
    assert_eq!(
        threads[0].url, None,
        "the service publishes no address for a review's own summary",
    );
}

#[test]
fn a_remark_on_the_pull_request_carries_where_it_can_be_read() {
    let payload: ReviewPayload = serde_json::from_str(
        r#"{"statusCheckRollup":[],"reviews":[],"comments":[{"id":"IC_1","author":{"login":"octocat"},"body":"closing this","url":"https://forge.example/pull/12#issuecomment-1"}]}"#,
    )
    .expect("parses");

    let (_, threads) = payload.into_parts();

    assert_eq!(
        threads[0].url.as_deref(),
        Some("https://forge.example/pull/12#issuecomment-1"),
    );
    assert_eq!(threads[0].path, None, "it hangs on no line of the diff");
}

#[test]
fn a_comment_whose_author_has_since_gone_still_reads_as_somebodys() {
    let payload: ReviewPayload = serde_json::from_str(
        r#"{"statusCheckRollup":[],"reviews":[],"comments":[{"id":"IC_1","author":null,"body":"a remark","url":"https://forge.example/x"}]}"#,
    )
    .expect("an author the service no longer has is not a broken answer");

    let (_, threads) = payload.into_parts();

    assert!(!threads[0].comments[0].author.is_empty());
}
