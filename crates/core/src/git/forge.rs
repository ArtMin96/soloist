//! The git context's second driven port: the hosting service a repository's pull requests live
//! on, plus the no-op default.
//!
//! Kept apart from [`GitRepository`](super::GitRepository) because it is a different machine
//! answering. A repository is on this disk and is always there; a forge is somebody else's
//! service, reached through a tool the user may not have installed and an account they may not
//! have signed in to — so the first thing this port answers is whether it can answer anything at
//! all ([`ForgeReadiness`]), and every surface asks that before it offers a single action.
//!
//! The port speaks only in domain values. Nothing the tool printed crosses it except a refusal's
//! own account, which is the service's answer to the user and is carried opaque, never read here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::exchange::{Progress, Stop};
use super::review::{CheckRun, MergeMethod, PullRequestReview, ReviewLimits};

/// Whether pull requests can be reached at all, and when not, which of the two fixable things is
/// in the way. A closed set: each answer has one thing the user can do about it, and a surface
/// says which.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeReadiness {
    /// The forge command-line tool is not installed, so nothing can be asked.
    Missing,
    /// It is installed but signed in to no account, so every request would be refused.
    LoggedOut,
    /// It is installed and signed in.
    Ready,
}

/// Where a pull request stands. A closed set, matching the three states a forge reports; an answer
/// naming any other is not understood rather than guessed at.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

/// One pull request, as a surface renders it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    /// Where it can be opened — the one thing a user reliably wants from a pull request they just
    /// made, and the only place the forge's own address appears.
    pub url: String,
    pub title: String,
    pub state: PullRequestState,
    /// Whether it is still marked as a draft, which is a different thing from being open.
    pub draft: bool,
    /// The branch it would merge into.
    pub base: String,
    /// The branch holding the commits it proposes.
    pub head: String,
}

/// What a new pull request is asked for with. The head branch is not here: it is whatever the
/// project has checked out, read from the repository rather than accepted from a caller, so no
/// surface can propose one branch's commits under another branch's name.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct NewPullRequest {
    pub title: String,
    /// The description, which is the user's own text: whatever a template seeded is theirs to
    /// change, and whatever an agent drafted was put in front of them before it got here.
    pub body: String,
    /// The branch to merge into. Named rather than left to the service's default, so what comes
    /// back is fully known from what was asked for — and so a surface has to show the user where
    /// their work is going before they send it.
    pub base: String,
    /// Whether to open it as a draft, so review is not requested yet.
    pub draft: bool,
}

/// What the service says about the repository itself, rather than about any one pull request.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ForgeRepository {
    /// The branch it merges into unless told otherwise, or `None` where the service does not say —
    /// which is what a repository with no commits yet answers.
    pub default_base: Option<String>,
    /// The ways it allows a pull request to be put into its base branch, the one it prefers first.
    /// Its own settings, so a surface offers what this repository actually permits rather than
    /// every method the service has.
    pub merge_methods: Vec<MergeMethod>,
}

/// One starting shape offered for a pull request's description, with the name a picker shows it
/// under. A repository offering several is offering a choice between them, so each keeps its name.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PullRequestTemplate {
    pub name: String,
    pub body: String,
}

/// Why a request to the forge produced no result.
///
/// Machine data and the service's own words, nothing in between: the cases a surface acts on
/// differently are named, and everything else keeps the exit status it had. The two fixable
/// states have their own case, because "install it" and "sign in" are different instructions.
#[derive(Clone, PartialEq, Eq, Debug, thiserror::Error)]
pub enum ForgeError {
    /// The forge command-line tool is not installed.
    #[error("the GitHub command-line tool is not installed")]
    Missing,
    /// It is installed but signed in to no account the request could be made as.
    #[error("the GitHub command-line tool is not signed in — run `gh auth login` and try again")]
    LoggedOut,
    /// The service refused. `output` is what it wrote about why — carried so a surface can show it,
    /// never read here, so no behaviour depends on the wording of a service we do not own.
    #[error("the forge refused")]
    Refused { output: String },
    /// The request did not finish within its time limit and was stopped.
    #[error("the forge did not answer within its time limit")]
    Timeout,
    /// Whoever asked for it asked for it to stop, and it did. Not a failure: it is what was asked
    /// for, and it is told apart from every other case so nothing reports it as one.
    #[error("the forge request was stopped")]
    Stopped,
    /// A failure none of the cases above names: a non-zero exit whose meaning nothing
    /// machine-readable carried, an answer that did not parse, or an operating-system error
    /// running the tool.
    #[error("the forge request failed")]
    Op { status: Option<i32> },
}

/// Reaches the hosting service a repository's pull requests live on.
///
/// An implementation is **blocking** and reaches another machine, so callers come from the
/// blocking pool ([`crate::facade::Facade::blocking`]) rather than a runtime worker. It must
/// return within a bounded time — a request that cannot finish is a [`ForgeError::Timeout`],
/// never a wait without end — and must leave no process behind.
///
/// Soloist stores no credential for it and never sees one: the tool underneath owns the account
/// entirely, which is the same bargain the repository port makes with the user's own `git`.
pub trait GitForge: Send + Sync {
    /// Whether the forge can be reached at all, asked from within `root` so a repository pointed at
    /// its own host is judged where it lives.
    ///
    /// Infallible by design: every way of failing to answer *is* an answer here — a tool that
    /// cannot be run is [`ForgeReadiness::Missing`] — so a surface never has to handle an error
    /// about whether there was an error.
    fn readiness(&self, root: &Path) -> ForgeReadiness;

    /// What the service says about the repository at `root` itself: what it merges into, and how it
    /// allows pull requests to be put there.
    fn repository(&self, root: &Path) -> Result<ForgeRepository, ForgeError>;

    /// The description skeletons the repository at `root` carries as its own convention, in the
    /// order a picker should offer them, or empty when it carries none.
    ///
    /// A repository's own convention, so it is read from the repository rather than asked of the
    /// service — which also means it answers for a repository nobody has pushed yet.
    fn templates(&self, root: &Path) -> Result<Vec<PullRequestTemplate>, ForgeError>;

    /// The pull request `branch` already has open on the service, or `None` when it has none —
    /// which is the ordinary state of a branch nobody has proposed yet, not a failure.
    fn pull_request(&self, root: &Path, branch: &str) -> Result<Option<PullRequest>, ForgeError>;

    /// Proposes `branch`'s commits as a new pull request, answering with the address of what was
    /// made.
    ///
    /// The address and nothing more, deliberately: it is what the service itself reports, so
    /// nothing here has to assemble a record out of what was asked for, and a caller that wants the
    /// whole of it reads it the way every other reader does. That is what keeps a proposal that was
    /// accepted from ever being reported as one that failed.
    ///
    /// The branch must already be on the service: publishing it is version control's job and is
    /// done before this is reached, so nothing here pushes anything or asks anybody where to.
    ///
    /// Bounded by the implementation's own limit for reaching a service, and stoppable before then:
    /// [`ForgeError::Stopped`] when `stop` was set, which is not a failure but the answer to what
    /// was asked.
    fn create(
        &self,
        root: &Path,
        branch: &str,
        new: &NewPullRequest,
        stop: &Stop,
    ) -> Result<String, ForgeError>;

    /// What `branch`'s open pull request looks like under review — the pull request itself, what the
    /// service's checks say about it, and what people have written on it — or `None` when the branch
    /// has nothing open.
    ///
    /// `limits` is the core's ceiling on how much of somebody else's discussion is carried, applied
    /// by the implementation to what it asks for as well as to what it returns, so a long argument
    /// costs a bounded request rather than a bounded slice of an unbounded one.
    fn review(
        &self,
        root: &Path,
        branch: &str,
        limits: ReviewLimits,
    ) -> Result<Option<PullRequestReview>, ForgeError>;

    /// Puts pull request `number`'s commits into its base branch by `method`.
    ///
    /// The service's own rules decide whether it may be: a required check that has not passed, a
    /// review that is owed, a base that has moved. Anything it refuses comes back as
    /// [`ForgeError::Refused`] carrying its account, and nothing is merged — there is no way to ask
    /// this to override a repository's rules, deliberately.
    ///
    /// Stoppable before the time limit, like every other request that reaches a service, and
    /// reported on while it runs where `progress` is watched.
    fn merge(
        &self,
        root: &Path,
        number: u64,
        method: MergeMethod,
        stop: &Stop,
        progress: &Progress,
    ) -> Result<(), ForgeError>;

    /// The tail of what `check` printed when it failed, at most `limit` bytes, or `None` when the
    /// check names nothing whose output this can reach.
    ///
    /// The tail rather than the head, because a failure is at the end of a log and the beginning of
    /// one is the machine describing itself. Whether an address names something with a log at all is
    /// the implementation's business — a check reported by something other than the service's own
    /// runner has no log here, and that is an ordinary answer rather than a failure.
    fn check_log(
        &self,
        root: &Path,
        check: &CheckRun,
        limit: usize,
    ) -> Result<Option<String>, ForgeError>;
}

/// A [`GitForge`] that reports the tool as absent — the default until the real adapter is wired
/// (headless tools, tests that do not exercise a forge).
///
/// It degrades **silently**, like every other optional driven port: a surface reads
/// [`ForgeReadiness::Missing`] as "there is nothing to offer here", so a core built without the
/// adapter behaves exactly as one on a machine where the tool was never installed.
#[derive(Clone, Copy, Default)]
pub struct NoopGitForge;

impl GitForge for NoopGitForge {
    fn readiness(&self, _root: &Path) -> ForgeReadiness {
        ForgeReadiness::Missing
    }

    fn repository(&self, _root: &Path) -> Result<ForgeRepository, ForgeError> {
        Err(ForgeError::Missing)
    }

    fn templates(&self, _root: &Path) -> Result<Vec<PullRequestTemplate>, ForgeError> {
        Err(ForgeError::Missing)
    }

    fn pull_request(&self, _root: &Path, _branch: &str) -> Result<Option<PullRequest>, ForgeError> {
        Err(ForgeError::Missing)
    }

    fn create(
        &self,
        _root: &Path,
        _branch: &str,
        _new: &NewPullRequest,
        _stop: &Stop,
    ) -> Result<String, ForgeError> {
        Err(ForgeError::Missing)
    }

    fn review(
        &self,
        _root: &Path,
        _branch: &str,
        _limits: ReviewLimits,
    ) -> Result<Option<PullRequestReview>, ForgeError> {
        Err(ForgeError::Missing)
    }

    fn merge(
        &self,
        _root: &Path,
        _number: u64,
        _method: MergeMethod,
        _stop: &Stop,
        _progress: &Progress,
    ) -> Result<(), ForgeError> {
        Err(ForgeError::Missing)
    }

    fn check_log(
        &self,
        _root: &Path,
        _check: &CheckRun,
        _limit: usize,
    ) -> Result<Option<String>, ForgeError> {
        Err(ForgeError::Missing)
    }
}
