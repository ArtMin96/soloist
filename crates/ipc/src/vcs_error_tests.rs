//! What the wire says about a version-control refusal — and, above all, that the ones a caller
//! must act on differently arrive as different words rather than different sentences.

use soloist_core::{ForgeError, GitError, GitWriteError, PullRequestError, ScopedGitError};

use super::GitRefusal;
use crate::error::IpcError;

/// The word and the sentence a refusal reaches the wire as.
fn wire(err: ScopedGitError) -> (GitRefusal, String) {
    match IpcError::from(err) {
        IpcError::Git { reason, message } => (reason, message),
        other => panic!("expected a version-control refusal, got {other:?}"),
    }
}

#[test]
fn an_operation_somebody_stopped_is_a_different_word_from_one_that_failed() {
    let (stopped, _) = wire(ScopedGitError::Change(GitWriteError::Git(
        GitError::Stopped,
    )));
    let (failed, _) = wire(ScopedGitError::Change(GitWriteError::Git(GitError::Op {
        status: Some(128),
    })));
    let (timed_out, _) = wire(ScopedGitError::Change(GitWriteError::Git(
        GitError::Timeout,
    )));

    assert_eq!(stopped, GitRefusal::Stopped);
    assert_ne!(
        stopped, failed,
        "being stopped is what was asked for, not a failure"
    );
    assert_ne!(
        stopped, timed_out,
        "somebody changing their mind is not the same as waiting too long"
    );
}

#[test]
fn a_proposal_stopped_while_the_service_was_answering_is_the_same_word() {
    // The two ports have refusal taxonomies of their own; a caller acts on being stopped the same
    // way whichever of them was in the middle of it.
    let (from_forge, _) = wire(ScopedGitError::PullRequest(PullRequestError::Forge(
        ForgeError::Stopped,
    )));

    assert_eq!(from_forge, GitRefusal::Stopped);
}

#[test]
fn the_project_trust_gate_is_its_own_word_rather_than_a_generic_refusal() {
    let (from_change, _) = wire(ScopedGitError::Change(GitWriteError::Untrusted));
    let (from_proposal, _) = wire(ScopedGitError::PullRequest(PullRequestError::Untrusted));

    assert_eq!(from_change, GitRefusal::ProjectUntrusted);
    assert_eq!(
        from_proposal,
        GitRefusal::ProjectUntrusted,
        "the same grant is refused by the same word wherever it is spent"
    );
}

#[test]
fn what_a_hook_wrote_reaches_the_caller_beside_the_word_that_classifies_it() {
    let (reason, message) = wire(ScopedGitError::Change(GitWriteError::Git(
        GitError::Refused {
            output: "commit-msg: subject must start with a verb".into(),
        },
    )));

    assert_eq!(reason, GitRefusal::Refused);
    assert!(
        message.contains("commit-msg: subject must start with a verb"),
        "the hook's own account is what names what is in the way: {message}"
    );
}

#[test]
fn the_two_fixable_forge_states_are_told_apart_because_the_remedies_differ() {
    let (missing, _) = wire(ScopedGitError::PullRequest(PullRequestError::Forge(
        ForgeError::Missing,
    )));
    let (logged_out, _) = wire(ScopedGitError::PullRequest(PullRequestError::Forge(
        ForgeError::LoggedOut,
    )));

    assert_eq!(missing, GitRefusal::ForgeMissing);
    assert_eq!(logged_out, GitRefusal::ForgeLoggedOut);
}

#[test]
fn a_push_that_failed_under_a_proposal_keeps_the_pushs_own_word() {
    let (reason, _) = wire(ScopedGitError::PullRequest(PullRequestError::Push(
        GitWriteError::Git(GitError::AuthFailed),
    )));

    assert_eq!(
        reason,
        GitRefusal::AuthFailed,
        "a proposal that could not publish the branch says why it could not"
    );
}

#[test]
fn a_session_with_no_scope_and_a_project_that_is_not_open_keep_the_wires_own_words() {
    // Neither is about version control, so neither becomes a version-control reason — a caller
    // already knows what to do about both, and they mean the same thing on every other surface.
    assert!(matches!(
        IpcError::from(ScopedGitError::NoProjectScope),
        IpcError::NoProjectScope,
    ));
    assert!(matches!(
        IpcError::from(ScopedGitError::Change(GitWriteError::UnknownProject)),
        IpcError::UnknownProject,
    ));
}

#[test]
fn every_version_control_refusal_reaches_the_caller_rather_than_being_swallowed() {
    // A refusal delivered as a server-side failure is one the model never sees; each of these is
    // something it must be able to report or act on.
    for err in [
        ScopedGitError::NotARepository,
        ScopedGitError::Change(GitWriteError::Untrusted),
        ScopedGitError::Change(GitWriteError::Git(GitError::Stopped)),
        ScopedGitError::Change(GitWriteError::Git(GitError::AuthFailed)),
        ScopedGitError::Change(GitWriteError::Git(GitError::GitMissing)),
        ScopedGitError::PullRequest(PullRequestError::NoPullRequest),
    ] {
        let mapped = IpcError::from(err);
        assert!(
            mapped.is_request_error(),
            "{mapped:?} must reach the caller as actionable feedback"
        );
    }
}

#[test]
fn a_durable_read_that_failed_is_not_reported_as_a_version_control_refusal() {
    let mapped = IpcError::from(ScopedGitError::Change(GitWriteError::Store(
        soloist_core::StoreError::Backend("disk full".into()),
    )));

    assert!(matches!(mapped, IpcError::Internal(_)));
    assert!(
        !mapped.is_request_error(),
        "nothing the caller did caused it, and no retry of theirs fixes it"
    );
}

#[test]
fn the_word_survives_the_round_trip_a_reply_makes() {
    let sent = IpcError::from(ScopedGitError::Change(GitWriteError::Git(
        GitError::Stopped,
    )));

    let json = serde_json::to_string(&sent).expect("serialize");
    let back: IpcError = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back, sent);
    assert!(
        json.contains("\"stopped\""),
        "the word a caller matches on is on the wire: {json}"
    );
}
