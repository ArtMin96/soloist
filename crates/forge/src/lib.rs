//! The pull-request adapter for Soloist (a driven adapter crate).
//!
//! Implements the core's [`GitForge`] port against the GitHub `gh` command line, so the pure core
//! never runs a subprocess itself. Driving the user's own `gh` is what makes their account, their
//! host, and their enterprise configuration apply by construction — and it is why **Soloist stores
//! no token, names no credential helper, and never sees a secret**: the tool owns the account
//! entirely, exactly as the repository adapter leaves credentials to the user's own `git`.
//!
//! Nothing here names a host. A repository pointed at an enterprise server is answered by the same
//! invocations, because deciding which service a repository belongs to is the tool's own job.
//!
//! The crate depends only on `soloist-core` and the operating system — never the reverse (the
//! dependency-direction guard enforces it).

mod gh;
mod json;
mod templates;

use std::path::Path;

use soloist_core::{
    ForgeError, ForgeReadiness, GitForge, NewPullRequest, PullRequest, PullRequestTemplate, Stop,
};

/// The arguments asking which accounts the tool holds, in the one machine-readable form it offers.
/// It answers with a zero status whether or not there is an account, so the payload is the whole
/// signal.
const AUTH_ARGS: &[&str] = &["auth", "status", "--json", "hosts"];

/// The arguments asking what the repository merges into by default.
const REPOSITORY_ARGS: &[&str] = &["repo", "view", "--json", "defaultBranchRef"];

/// How many pull requests are asked for when the question is "does this branch have one". One,
/// because a branch's most recent proposal is the only one a surface has anything to say about.
const NEWEST: &str = "1";

/// Reaches pull requests by running the GitHub `gh` command line.
#[derive(Clone, Copy, Default)]
pub struct GhForge;

impl GhForge {
    /// A forge reader over whichever `gh` the user has installed.
    pub fn new() -> Self {
        Self
    }
}

impl GitForge for GhForge {
    fn readiness(&self, root: &Path) -> ForgeReadiness {
        match gh::run(root, AUTH_ARGS) {
            Ok(output) if json::signed_in(&output) => ForgeReadiness::Ready,
            Ok(_) => ForgeReadiness::LoggedOut,
            Err(ForgeError::Missing) => ForgeReadiness::Missing,
            // Installed, but it could not say what it holds. Reporting no account is the safe way
            // round: the surface then offers the one thing that could fix it.
            Err(_) => ForgeReadiness::LoggedOut,
        }
    }

    fn default_base(&self, root: &Path) -> Result<Option<String>, ForgeError> {
        json::default_base(&gh::run(root, REPOSITORY_ARGS)?)
    }

    fn templates(&self, root: &Path) -> Result<Vec<PullRequestTemplate>, ForgeError> {
        Ok(templates::detect(root))
    }

    fn pull_request(&self, root: &Path, branch: &str) -> Result<Option<PullRequest>, ForgeError> {
        // Asked as a list rather than as a view of one, because a list answers "there is none"
        // with an empty list and a zero status, where a view answers it with a failure that is
        // told from every other failure only by its prose.
        let output = gh::run(
            root,
            &[
                "pr",
                "list",
                "--head",
                branch,
                "--state",
                "all",
                "--limit",
                NEWEST,
                "--json",
                json::PR_FIELDS,
            ],
        )?;
        json::first_pull_request(&output)
    }

    fn create(
        &self,
        root: &Path,
        branch: &str,
        new: &NewPullRequest,
        stop: &Stop,
    ) -> Result<String, ForgeError> {
        let stopped = || stop.stopped();
        // `--head` names the branch outright, which is also what stops the tool from offering to
        // push it or to fork: the branch is already on the remote, put there by version control
        // before this was reached. The description goes over standard input, because a person's
        // prose has no business on a command line.
        let mut args = vec![
            "pr",
            "create",
            "--head",
            branch,
            "--base",
            &new.base,
            "--title",
            &new.title,
            "--body-file",
            STANDARD_INPUT,
        ];
        if new.draft {
            args.push("--draft");
        }
        let output = gh::run_with(
            root,
            &args,
            gh::Run {
                input: Some(&new.body),
                stopped: Some(&stopped),
            },
        )?;
        address(&output)
    }
}

/// What the tool reads a file argument as when the file is standard input.
const STANDARD_INPUT: &str = "-";

/// The address a create printed: what it made, and the only thing it prints.
///
/// The last line rather than the whole of it, because anything the tool has to say about what it is
/// doing comes first and the address comes last — and an answer with no address at all is a
/// failure, since a create that produced nothing to open produced nothing at all.
fn address(output: &[u8]) -> Result<String, ForgeError> {
    String::from_utf8_lossy(output)
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_string)
        .ok_or(ForgeError::Op { status: None })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
