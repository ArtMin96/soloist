//! Unit tests for telling a failure to authenticate apart from every other way an exchange with a
//! remote can fail, and for what version control is actually asked to do. Whether a real remote is
//! reached at all is `tests/sync.rs`; this pins the one decision made from what version control
//! wrote — because there is nothing machine-readable to make it from — and the arguments each
//! exchange is invoked with.

use soloist_core::SyncOp;

use super::{authentication_failed, invocation};

#[test]
fn a_credential_that_could_not_be_asked_for_is_a_failure_to_authenticate() {
    assert!(authentication_failed(
        "fatal: could not read Username for 'https://example.com': terminal prompts disabled"
    ));
}

#[test]
fn a_credential_the_remote_turned_down_is_too() {
    assert!(authentication_failed(
        "remote: Invalid username or token\nfatal: Authentication failed for 'https://example.com/'"
    ));
    assert!(authentication_failed(
        "git@example.com: Permission denied (publickey).\nfatal: Could not read from remote \
         repository."
    ));
}

#[test]
fn a_remote_that_cannot_be_reached_is_reported_as_itself_rather_than_as_a_credential() {
    assert!(
        !authentication_failed(
            "fatal: unable to access 'http://127.0.0.1:1/repo.git/': Failed to connect to \
             127.0.0.1 port 1 after 0 ms: Could not connect to server"
        ),
        "an unreachable remote shares its exit status with a refused credential, so a wrong \
         classification here would send the reader looking for the wrong problem",
    );
    assert!(!authentication_failed(
        "hint: You have divergent branches and need to specify how to reconcile them.\nfatal: \
         Need to specify how to reconcile divergent branches."
    ));
}

#[test]
fn an_exchange_nobody_asked_about_is_invoked_exactly_as_it_was_before_progress_existed() {
    let unasked = [
        (SyncOp::Fetch, vec!["fetch"]),
        (SyncOp::Pull, vec!["pull"]),
        (SyncOp::Push, vec!["push"]),
        (
            SyncOp::Publish,
            vec!["push", "--set-upstream", "origin", "HEAD"],
        ),
    ];

    for (op, expected) in unasked {
        assert_eq!(
            invocation(op, false),
            expected,
            "nothing about {op:?} may change for a caller that never asked to be told",
        );
    }
}

#[test]
fn an_exchange_somebody_asked_about_asks_version_control_to_describe_itself() {
    for op in [SyncOp::Fetch, SyncOp::Pull, SyncOp::Push, SyncOp::Publish] {
        let asked = invocation(op, true);

        assert_eq!(
            asked.len(),
            invocation(op, false).len() + 1,
            "asking to be told changed more about {op:?} than asking to be told",
        );
        assert!(
            asked.contains(&"--progress"),
            "{op:?} was not asked to describe itself: {asked:?}",
        );
    }
}
