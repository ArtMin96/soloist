//! Unit tests for telling a failure to authenticate apart from every other way an exchange with a
//! remote can fail. Whether a real remote is reached at all is `tests/sync.rs`; this pins the one
//! decision made from what version control wrote, because there is nothing machine-readable to make
//! it from.

use super::authentication_failed;

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
