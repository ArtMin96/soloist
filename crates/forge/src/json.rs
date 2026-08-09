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

use soloist_core::{ForgeError, PullRequest, PullRequestState};

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

/// What the tool reports about the repository itself.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryPayload {
    default_branch_ref: Option<NamedRef>,
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

/// The branch a repository merges into unless told otherwise, or `None` where the tool does not say
/// — which is what a repository with no commits yet answers.
pub(crate) fn default_base(output: &[u8]) -> Result<Option<String>, ForgeError> {
    let repository: RepositoryPayload = from_slice(output).map_err(|_| unreadable())?;
    Ok(repository.default_branch_ref.map(|head| head.name))
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
