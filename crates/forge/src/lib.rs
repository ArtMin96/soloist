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
mod log;
mod review;
mod templates;
mod threads;

use std::path::Path;

use soloist_core::{
    CheckRun, ForgeError, ForgeReadiness, ForgeRepository, GitForge, MergeMethod, NewPullRequest,
    Progress, PullRequest, PullRequestReview, PullRequestTemplate, ReviewLimits, Stop,
};
use soloist_exec::{Watch, REPORT_INTERVAL};

/// The arguments asking which accounts the tool holds, in the one machine-readable form it offers.
/// It answers with a zero status whether or not there is an account, so the payload is the whole
/// signal.
const AUTH_ARGS: &[&str] = &["auth", "status", "--json", "hosts"];

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

    fn repository(&self, root: &Path) -> Result<ForgeRepository, ForgeError> {
        json::repository(&gh::run(
            root,
            &["repo", "view", "--json", json::REPOSITORY_FIELDS],
        )?)
    }

    fn templates(&self, root: &Path) -> Result<Vec<PullRequestTemplate>, ForgeError> {
        Ok(templates::detect(root))
    }

    fn pull_request(&self, root: &Path, branch: &str) -> Result<Option<PullRequest>, ForgeError> {
        json::first_pull_request(&gh::run(root, &listing(branch, json::PR_FIELDS))?)
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
                watching: None,
            },
        )?;
        address(&output)
    }

    fn review(
        &self,
        root: &Path,
        branch: &str,
        limits: ReviewLimits,
    ) -> Result<Option<PullRequestReview>, ForgeError> {
        let fields = format!("{},{}", json::PR_FIELDS, review::REVIEW_FIELDS);
        let Some(mut reviewed) = json::first_review(&gh::run(root, &listing(branch, &fields))?)?
        else {
            return Ok(None);
        };
        // The conversations that hang on lines of the diff are the one thing the pull-request
        // commands do not report, so they are asked for separately — of the service the pull
        // request's own address names.
        if let Some(host) = threads::host_of(&reviewed.pull_request.url) {
            let args = threads::args(
                host,
                reviewed.pull_request.number,
                limits.threads,
                limits.comments,
            );
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            let inline = threads::threads(&gh::run(root, &borrowed)?)?;
            // Inline first: a comment on a line is what a reader came to read, and a review's own
            // summary is the frame around it.
            reviewed.threads = inline.into_iter().chain(reviewed.threads).collect();
        }
        reviewed.threads.truncate(limits.threads);
        for thread in &mut reviewed.threads {
            thread.comments.truncate(limits.comments);
        }
        Ok(Some(reviewed))
    }

    fn merge(
        &self,
        root: &Path,
        number: u64,
        method: MergeMethod,
        stop: &Stop,
        progress: &Progress,
    ) -> Result<(), ForgeError> {
        let stopped = || stop.stopped();
        let report = |remark: &str| progress.report(remark);
        let number = number.to_string();
        gh::run_with(
            root,
            &["pr", "merge", &number, merge_flag(method)],
            gh::Run {
                input: None,
                stopped: Some(&stopped),
                watching: progress.is_watched().then_some(Watch {
                    interval: REPORT_INTERVAL,
                    observer: &report,
                }),
            },
        )?;
        Ok(())
    }

    fn check_log(
        &self,
        root: &Path,
        check: &CheckRun,
        limit: usize,
    ) -> Result<Option<String>, ForgeError> {
        let Some(url) = &check.url else {
            return Ok(None);
        };
        let Some(job) = log::job_of(url) else {
            return Ok(None);
        };
        let args = log::args(job);
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        Ok(Some(log::tail(&gh::run(root, &borrowed)?, limit)))
    }
}

/// The arguments listing `branch`'s most recent pull request, asking for `fields`.
///
/// Asked as a list rather than as a view of one, because a list answers "there is none" with an
/// empty list and a zero status, where a view answers it with a failure that is told from every
/// other failure only by its prose.
fn listing<'a>(branch: &'a str, fields: &'a str) -> Vec<&'a str> {
    vec![
        "pr", "list", "--head", branch, "--state", "all", "--limit", NEWEST, "--json", fields,
    ]
}

/// The flag naming each way of putting a pull request into its base. Always passed: with nobody at
/// a terminal the tool has no way to ask which was meant, and a merge is not something to leave to
/// a default.
fn merge_flag(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "--merge",
        MergeMethod::Squash => "--squash",
        MergeMethod::Rebase => "--rebase",
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
