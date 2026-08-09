//! Turning the tool's `--json` payloads into the core's pull-request vocabulary.
//!
//! Every field asked for is named here and nowhere else, so what is requested and what is read
//! cannot drift apart. Parsing is tolerant in one direction only: a field the tool adds is ignored,
//! while one it was asked for and did not send is a failure rather than a guess — an answer that
//! did not parse is not an answer.

use serde::de::IgnoredAny;
use serde::Deserialize;
use serde_json::from_slice;
use std::collections::BTreeMap;

use soloist_core::{
    ForgeError, ForgeRepository, MergeMethod, PullRequest, PullRequestReview, PullRequestState,
};

use crate::review::ReviewPayload;

/// The fields one pull request is asked for, in the tool's own spelling. Requested as one string
/// and read into [`Payload`], so the request and the reading are the same list.
pub(crate) const PR_FIELDS: &str = "number,url,title,state,isDraft,baseRefName,headRefName";

/// What the tool calls each state it reports. Its own spelling, matched exactly: a fourth would be
/// a change to what a pull request can be, which is worth failing on rather than guessing past.
const OPEN: &str = "OPEN";
const CLOSED: &str = "CLOSED";
const MERGED: &str = "MERGED";

/// One pull request as the tool reports it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    number: u64,
    url: String,
    title: String,
    state: String,
    is_draft: bool,
    base_ref_name: String,
    head_ref_name: String,
}

/// The fields the repository itself is asked for, in the tool's own spelling.
pub(crate) const REPOSITORY_FIELDS: &str = "defaultBranchRef,mergeCommitAllowed,squashMergeAllowed,rebaseMergeAllowed,viewerDefaultMergeMethod";

/// What the tool calls each way of putting a pull request into its base. Its own spelling, matched
/// exactly; a word this does not know simply orders nothing, since the ways themselves are read
/// from the three flags beside it.
const MERGE: &str = "MERGE";
const SQUASH: &str = "SQUASH";
const REBASE: &str = "REBASE";

/// What the tool reports about the repository itself.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryPayload {
    default_branch_ref: Option<NamedRef>,
    merge_commit_allowed: bool,
    squash_merge_allowed: bool,
    rebase_merge_allowed: bool,
    viewer_default_merge_method: String,
}

/// One pull request as the tool reports it, together with what it says about its review. Read as
/// one shape because it arrives as one answer.
#[derive(Deserialize)]
struct ReviewedPayload {
    #[serde(flatten)]
    pull_request: Payload,
    #[serde(flatten)]
    review: ReviewPayload,
}

#[derive(Deserialize)]
struct NamedRef {
    name: String,
}

/// What the tool reports about the accounts it holds: one list per host, of which only the
/// presence of an entry is read.
#[derive(Deserialize)]
struct AuthPayload {
    hosts: BTreeMap<String, Vec<IgnoredAny>>,
}

/// Whether the tool holds an account on any host.
///
/// An answer that did not parse is read as holding none, which is the safe way round: the surface
/// then says to sign in, where claiming an account there is no evidence of would leave every
/// request to fail one at a time.
pub(crate) fn signed_in(output: &[u8]) -> bool {
    from_slice::<AuthPayload>(output)
        .is_ok_and(|auth| auth.hosts.values().any(|accounts| !accounts.is_empty()))
}

/// What the tool says about the repository itself: what it merges into, and how it allows a pull
/// request to be put there.
pub(crate) fn repository(output: &[u8]) -> Result<ForgeRepository, ForgeError> {
    let repository: RepositoryPayload = from_slice(output).map_err(|_| unreadable())?;
    let allowed = [
        (MergeMethod::Merge, repository.merge_commit_allowed),
        (MergeMethod::Squash, repository.squash_merge_allowed),
        (MergeMethod::Rebase, repository.rebase_merge_allowed),
    ];
    let preferred = merge_method(&repository.viewer_default_merge_method);
    let mut merge_methods: Vec<MergeMethod> = allowed
        .into_iter()
        .filter_map(|(method, permitted)| permitted.then_some(method))
        .collect();
    // The one it prefers goes first, so a surface offering these offers the repository's own answer
    // without having to know which that is.
    merge_methods.sort_by_key(|method| Some(*method) != preferred);
    Ok(ForgeRepository {
        default_base: repository.default_branch_ref.map(|head| head.name),
        merge_methods,
    })
}

/// Which way of merging a word names, or `None` for one this does not know.
fn merge_method(reported: &str) -> Option<MergeMethod> {
    match reported {
        MERGE => Some(MergeMethod::Merge),
        SQUASH => Some(MergeMethod::Squash),
        REBASE => Some(MergeMethod::Rebase),
        _ => None,
    }
}

/// The first listed pull request with what the tool says about its review, or `None` when the list
/// is empty. The threads that hang on lines of the diff are not here — the tool does not report
/// them — so what comes back carries only the conversations about the change as a whole.
pub(crate) fn first_review(output: &[u8]) -> Result<Option<PullRequestReview>, ForgeError> {
    let listed: Vec<ReviewedPayload> = from_slice(output).map_err(|_| unreadable())?;
    let Some(first) = listed.into_iter().next() else {
        return Ok(None);
    };
    let (checks, threads) = first.review.into_parts();
    Ok(Some(PullRequestReview {
        pull_request: pull_request(first.pull_request)?,
        checks,
        threads,
    }))
}

/// The first pull request of a list, or `None` when the list is empty — which is the ordinary
/// answer for a branch nobody has proposed yet.
pub(crate) fn first_pull_request(output: &[u8]) -> Result<Option<PullRequest>, ForgeError> {
    let listed: Vec<Payload> = from_slice(output).map_err(|_| unreadable())?;
    listed.into_iter().next().map(pull_request).transpose()
}

/// One pull request from what the tool reported about it.
fn pull_request(payload: Payload) -> Result<PullRequest, ForgeError> {
    Ok(PullRequest {
        number: payload.number,
        url: payload.url,
        title: payload.title,
        state: state(&payload.state).ok_or_else(unreadable)?,
        draft: payload.is_draft,
        base: payload.base_ref_name,
        head: payload.head_ref_name,
    })
}

/// Where a pull request stands, or `None` for a word this does not know.
fn state(reported: &str) -> Option<PullRequestState> {
    match reported {
        OPEN => Some(PullRequestState::Open),
        CLOSED => Some(PullRequestState::Closed),
        MERGED => Some(PullRequestState::Merged),
        _ => None,
    }
}

/// What an answer that could not be read reports: a failure carrying no status, because the
/// invocation itself succeeded and it is the answer that is the problem.
fn unreadable() -> ForgeError {
    ForgeError::Op { status: None }
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
