//! Reading the conversations that hang on a line of the diff.
//!
//! These are the one thing the tool's pull-request commands do not report: its `--json` surface
//! carries reviews and remarks, but not which line a comment hangs on, and not whether anybody has
//! marked the argument settled. Both are what a reader needs most, so this asks the service's own
//! query interface for them through the tool's escape hatch — still the tool's account, still the
//! tool's credentials, still no token here.
//!
//! **Which service is asked comes from the pull request's own address**, not from a name written
//! here. The escape hatch resolves its host from the account the tool holds by default rather than
//! from the repository, which for somebody signed in to two services would be the wrong one; naming
//! the host the service itself reported is what makes an enterprise repository answer for itself.
//! Nothing in this crate names a host.

use serde::Deserialize;

use soloist_core::{ForgeError, ReviewComment, ReviewThread};

/// The query asked. Every field it names is read into [`ThreadsPayload`], so what is requested and
/// what is read cannot drift apart. Both `first:` counts are supplied by the caller from the core's
/// own ceiling, so a long argument costs a bounded request rather than a bounded slice of an
/// unbounded one.
const QUERY: &str = "\
query($owner:String!,$repo:String!,$number:Int!,$threads:Int!,$comments:Int!){\
repository(owner:$owner,name:$repo){\
pullRequest(number:$number){\
reviewThreads(first:$threads){nodes{\
id isResolved isOutdated path line \
comments(first:$comments){nodes{author{login} body url}}\
}}}}}";

/// What the escape hatch replaces with the repository of the folder it runs in — the tool's own
/// placeholders, so which repository is asked about is its job rather than something parsed here.
const OWNER_PLACEHOLDER: &str = "owner={owner}";
const REPO_PLACEHOLDER: &str = "repo={repo}";

/// What the query answers with, down to the nodes worth reading.
#[derive(Deserialize)]
struct ThreadsPayload {
    data: RepositoryData,
}

#[derive(Deserialize)]
struct RepositoryData {
    repository: PullRequestData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestData {
    pull_request: ThreadNodes,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadNodes {
    review_threads: Nodes<ThreadPayload>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadPayload {
    id: String,
    is_resolved: bool,
    is_outdated: bool,
    path: Option<String>,
    line: Option<u64>,
    comments: Nodes<CommentPayload>,
}

#[derive(Deserialize)]
struct CommentPayload {
    author: Option<Author>,
    body: String,
    url: String,
}

#[derive(Deserialize)]
struct Author {
    login: String,
}

/// What an author with no account left is shown as.
const NOBODY: &str = "(unknown)";

/// The arguments asking for `number`'s inline conversations on the service `host` names, at most
/// `threads` of them carrying at most `comments` each.
pub(crate) fn args(host: &str, number: u64, threads: usize, comments: usize) -> Vec<String> {
    vec![
        "api".to_string(),
        "graphql".to_string(),
        "--hostname".to_string(),
        host.to_string(),
        "-F".to_string(),
        OWNER_PLACEHOLDER.to_string(),
        "-F".to_string(),
        REPO_PLACEHOLDER.to_string(),
        "-F".to_string(),
        format!("number={number}"),
        "-F".to_string(),
        format!("threads={threads}"),
        "-F".to_string(),
        format!("comments={comments}"),
        "-f".to_string(),
        format!("query={QUERY}"),
    ]
}

/// The host to ask, taken from the address the service itself reported for the pull request.
///
/// A URL's authority and nothing else: everything before the first `/` after the scheme. `None`
/// where the address is not one this can read, which leaves the conversations unread rather than
/// asking the wrong service.
pub(crate) fn host_of(url: &str) -> Option<&str> {
    let after_scheme = url.split_once("://")?.1;
    let host = after_scheme.split('/').next()?;
    (!host.is_empty()).then_some(host)
}

/// The conversations the answer carries, in the order the service gave them.
pub(crate) fn threads(output: &[u8]) -> Result<Vec<ReviewThread>, ForgeError> {
    let payload: ThreadsPayload =
        serde_json::from_slice(output).map_err(|_| ForgeError::Op { status: None })?;
    Ok(payload
        .data
        .repository
        .pull_request
        .review_threads
        .nodes
        .into_iter()
        .map(thread)
        .collect())
}

fn thread(payload: ThreadPayload) -> ReviewThread {
    let comments: Vec<ReviewComment> = payload
        .comments
        .nodes
        .into_iter()
        .map(|comment| ReviewComment {
            author: comment
                .author
                .map_or_else(|| NOBODY.to_string(), |author| author.login),
            body: comment.body,
            url: Some(comment.url),
        })
        .collect();
    ReviewThread {
        id: payload.id,
        // Where the thread can be read is where its first comment is: the service publishes an
        // address per comment and none for the thread itself.
        url: comments.first().and_then(|first| first.url.clone()),
        path: payload.path,
        line: payload.line,
        resolved: payload.is_resolved,
        outdated: payload.is_outdated,
        comments,
    }
}

#[cfg(test)]
#[path = "threads_tests.rs"]
mod tests;
