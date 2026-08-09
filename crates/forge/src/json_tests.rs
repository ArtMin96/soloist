//! Tests for reading the tool's machine-readable answers. Every payload here was captured from a
//! real `gh` on a real repository and then scrubbed of anything naming a person or a project.

use super::{first_pull_request, repository, signed_in};
use soloist_core::{ForgeError, MergeMethod, PullRequestState};

/// What `gh auth status --json hosts` prints for an account that is signed in.
const SIGNED_IN: &[u8] = br#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"octocat","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"}]}}"#;

/// And for one that is not — a zero status either way, so the payload is the whole signal.
const SIGNED_OUT: &[u8] = br#"{"hosts":{}}"#;

/// What `gh pr list --json …` prints for a branch that has one.
const ONE_PROPOSAL: &[u8] = br#"[{"baseRefName":"main","headRefName":"feature","isDraft":true,"number":148,"state":"OPEN","title":"Propose the thing","url":"https://github.example/owner/repo/pull/148"}]"#;

#[test]
fn an_account_on_any_host_is_what_being_signed_in_means() {
    assert!(signed_in(SIGNED_IN));
    assert!(!signed_in(SIGNED_OUT));
}

#[test]
fn an_answer_that_could_not_be_read_is_taken_as_no_account_rather_than_as_one() {
    assert!(
        !signed_in(b"not json at all"),
        "claiming an account there is no evidence of would leave every request to fail one at a \
         time, where saying so offers the one thing that fixes it",
    );
}

#[test]
fn a_proposal_is_read_with_everything_a_surface_shows_about_it() {
    let proposal = first_pull_request(ONE_PROPOSAL)
        .expect("read")
        .expect("one proposal");

    assert_eq!(proposal.number, 148);
    assert_eq!(proposal.url, "https://github.example/owner/repo/pull/148");
    assert_eq!(proposal.title, "Propose the thing");
    assert_eq!(proposal.state, PullRequestState::Open);
    assert!(proposal.draft);
    assert_eq!(proposal.base, "main");
    assert_eq!(proposal.head, "feature");
}

#[test]
fn a_branch_nobody_has_proposed_yet_reads_as_having_none_rather_than_as_a_failure() {
    assert_eq!(first_pull_request(b"[]").expect("read"), None);
}

#[test]
fn every_state_the_service_reports_is_recognised_and_nothing_else_is() {
    for (reported, expected) in [
        ("OPEN", PullRequestState::Open),
        ("CLOSED", PullRequestState::Closed),
        ("MERGED", PullRequestState::Merged),
    ] {
        let payload = String::from_utf8(ONE_PROPOSAL.to_vec())
            .expect("text")
            .replace("\"OPEN\"", &format!("\"{reported}\""));
        let proposal = first_pull_request(payload.as_bytes())
            .expect("read")
            .expect("one proposal");
        assert_eq!(proposal.state, expected);
    }

    let unknown = String::from_utf8(ONE_PROPOSAL.to_vec())
        .expect("text")
        .replace("\"OPEN\"", "\"SOMETHING_ELSE\"");
    assert!(
        matches!(
            first_pull_request(unknown.as_bytes()),
            Err(ForgeError::Op { status: None }),
        ),
        "a fourth state is a change to what a pull request can be, which is worth failing on \
         rather than guessing past",
    );
}

#[test]
fn a_field_the_tool_adds_is_ignored_rather_than_treated_as_a_broken_answer() {
    let extended = String::from_utf8(ONE_PROPOSAL.to_vec())
        .expect("text")
        .replace(
            "\"number\":148",
            "\"number\":148,\"somethingNew\":{\"a\":1}",
        );

    assert!(first_pull_request(extended.as_bytes()).is_ok());
}

#[test]
fn a_field_that_was_asked_for_and_did_not_arrive_is_a_failure_rather_than_a_guess() {
    let missing = String::from_utf8(ONE_PROPOSAL.to_vec())
        .expect("text")
        .replace("\"number\":148,", "");

    assert!(matches!(
        first_pull_request(missing.as_bytes()),
        Err(ForgeError::Op { status: None }),
    ));
}

#[test]
fn a_repository_reports_the_branch_it_merges_into_and_says_so_when_it_has_none() {
    assert_eq!(
        repository(&allowing(br#""defaultBranchRef":{"name":"main"}"#))
            .expect("read")
            .default_base,
        Some("main".to_string()),
    );
    assert_eq!(
        repository(&allowing(br#""defaultBranchRef":null"#))
            .expect("read")
            .default_base,
        None,
        "a repository with no commits yet has no default branch, which is a state rather than a \
         failure",
    );
}

/// A repository payload allowing every way of merging, so a test about one field states only it.
fn allowing(head: &[u8]) -> Vec<u8> {
    let mut payload = b"{".to_vec();
    payload.extend_from_slice(head);
    payload.extend_from_slice(
        br#","mergeCommitAllowed":true,"squashMergeAllowed":true,"rebaseMergeAllowed":true,"viewerDefaultMergeMethod":"MERGE"}"#,
    );
    payload
}

#[test]
fn a_repository_offers_only_the_ways_of_merging_it_permits_its_own_first() {
    let read = repository(
        br#"{"defaultBranchRef":{"name":"main"},"mergeCommitAllowed":false,"squashMergeAllowed":true,"rebaseMergeAllowed":true,"viewerDefaultMergeMethod":"REBASE"}"#,
    )
    .expect("read");

    assert_eq!(
        read.merge_methods,
        vec![MergeMethod::Rebase, MergeMethod::Squash],
        "a repository that forbids merge commits must not be offered one, and the one it prefers \
         is what a surface should reach for first",
    );
}

#[test]
fn a_preference_this_does_not_recognise_still_leaves_every_permitted_way_on_offer() {
    let read = repository(
        br#"{"defaultBranchRef":null,"mergeCommitAllowed":true,"squashMergeAllowed":false,"rebaseMergeAllowed":false,"viewerDefaultMergeMethod":"SOMETHING_NEW"}"#,
    )
    .expect("read");

    assert_eq!(read.merge_methods, vec![MergeMethod::Merge]);
}
