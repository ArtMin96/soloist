//! Behavioural tests for composing a handoff — what an agent is told about a failing check or a
//! comment, and what it is never told.
//!
//! Everything asserted here is the composed text; delivery as a semantic turn is tested at the
//! façade seam.

use std::path::Path;
use std::sync::Arc;

use crate::git::NoopFileOpener;
use crate::git::{
    CheckState, HandoffSubject, PullRequestError, CHECK_LOG_LIMIT, HANDOFF_LIMIT, REVIEW_LIMITS,
};
use crate::ids::ProjectId;
use crate::testing::{
    check_run, git_status, pull_request_review, review_thread, FakeGitForge, FakeGitRepository,
    FakeTrustRepo,
};

use super::Git;

const ROOT: &str = "/project";
const BRANCH: &str = "feature";
const CHECK: &str = "build";
const THREAD: &str = "t1";

/// The git context over both fakes. Composing a handoff is a read, so nothing here is trusted.
fn git_with(forge: FakeGitForge) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(FakeGitRepository::reporting(git_status(BRANCH))),
        Arc::new(forge),
        Arc::new(NoopFileOpener),
        Arc::new(FakeTrustRepo::new()),
    ))
}

/// A forge whose branch has one failing check and one conversation open on it.
fn reviewed(log: Option<&str>) -> FakeGitForge {
    FakeGitForge::ready()
        .reviewing(pull_request_review(
            BRANCH,
            vec![check_run(CHECK, CheckState::Failed)],
            vec![review_thread(
                THREAD,
                "src/main.rs",
                42,
                "this leaks a file handle",
            )],
        ))
        .logging(log)
}

fn compose(forge: FakeGitForge, subject: HandoffSubject) -> Result<String, PullRequestError> {
    let project = ProjectId::next();
    git_with(forge).handoff_context(project, Path::new(ROOT), &subject)
}

#[test]
fn a_failing_check_is_handed_over_with_what_it_printed() {
    let text = compose(
        reviewed(Some("compiling\nerror: the thing is wrong\n")),
        HandoffSubject::Check {
            name: CHECK.to_string(),
        },
    )
    .expect("compose");

    assert!(text.contains(CHECK), "it names the check: {text}");
    assert!(
        text.contains("error: the thing is wrong"),
        "it carries what the check printed: {text}",
    );
    assert!(
        text.contains("#12"),
        "it says which pull request this is about: {text}",
    );
}

#[test]
fn a_check_whose_output_cannot_be_reached_says_so_rather_than_arriving_empty() {
    let text = compose(
        reviewed(None),
        HandoffSubject::Check {
            name: CHECK.to_string(),
        },
    )
    .expect("compose");

    assert!(text.contains(CHECK));
    assert!(
        text.contains("not reachable"),
        "a check with no reachable output must not read as one that printed nothing: {text}",
    );
}

#[test]
fn a_checks_output_is_asked_for_under_the_ceiling_the_core_sets() {
    let forge = reviewed(Some("some output"));
    let project = ProjectId::next();

    git_with(forge.clone())
        .handoff_context(
            project,
            Path::new(ROOT),
            &HandoffSubject::Check {
                name: CHECK.to_string(),
            },
        )
        .expect("compose");

    assert_eq!(
        forge.log_limits(),
        vec![CHECK_LOG_LIMIT],
        "how much of somebody else's output is carried is the core's bound, not the adapter's",
    );
}

#[test]
fn a_comment_is_handed_over_with_where_in_the_change_it_hangs() {
    let text = compose(
        reviewed(None),
        HandoffSubject::Thread {
            id: THREAD.to_string(),
        },
    )
    .expect("compose");

    assert!(
        text.contains("src/main.rs:42"),
        "a comment without its file and line is not the comment: {text}",
    );
    assert!(text.contains("this leaks a file handle"), "{text}");
    assert!(text.contains("octocat"), "it says who wrote it: {text}");
}

#[test]
fn a_subject_that_is_no_longer_on_the_pull_request_is_refused_rather_than_composed_from_nothing() {
    let refused = compose(
        reviewed(None),
        HandoffSubject::Check {
            name: "a check nobody ran".to_string(),
        },
    )
    .expect_err("there is no such check");

    assert!(matches!(refused, PullRequestError::NoSuchSubject));
}

#[test]
fn a_branch_with_nothing_open_has_nothing_to_hand_over() {
    let refused = compose(
        FakeGitForge::ready(),
        HandoffSubject::Check {
            name: CHECK.to_string(),
        },
    )
    .expect_err("nothing is open on this branch");

    assert!(matches!(refused, PullRequestError::NoPullRequest));
}

#[test]
fn a_conversation_longer_than_there_is_room_for_is_still_handed_over_whole_lines() {
    // A long argument is the ordinary case rather than the pathological one: twenty comments is
    // what the read's own ceiling permits, and twenty long ones is more than one turn can carry.
    let mut long = review_thread(THREAD, "src/main.rs", 42, "the first word on it");
    let said = "a paragraph of review that goes on at some length about the change".repeat(20);
    long.comments = std::iter::repeat_with(|| {
        let mut comment = long.comments[0].clone();
        comment.body = said.clone();
        comment
    })
    .take(REVIEW_LIMITS.comments)
    .collect();
    let forge =
        FakeGitForge::ready().reviewing(pull_request_review(BRANCH, Vec::new(), vec![long]));

    let text = compose(
        forge,
        HandoffSubject::Thread {
            id: THREAD.to_string(),
        },
    )
    .expect("compose");

    assert!(
        text.len() <= HANDOFF_LIMIT,
        "composed to the ceiling rather than past it: {} bytes",
        text.len(),
    );
    assert!(
        text.trim_end().ends_with("--- end of context ---"),
        "the fence closes even when the room ran out: {}",
        &text[text.len().saturating_sub(80)..],
    );
    assert!(
        text.contains("src/main.rs:42"),
        "what the handoff is about survives a conversation too long to carry whole",
    );
}
