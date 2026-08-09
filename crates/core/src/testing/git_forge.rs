//! A [`GitForge`] fake for the pull-request tests: it answers from what a test set on it without
//! reaching anything, and keeps every proposal it was handed — which is the only trace proposing
//! one leaves when there is no service underneath.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::git::{
    CheckRun, ForgeError, ForgeReadiness, ForgeRepository, GitForge, MergeMethod, NewPullRequest,
    Progress, PullRequest, PullRequestReview, PullRequestTemplate, ReviewLimits, Stop,
};
use crate::sync::lock;

/// How often a stalled proposal looks at whether it has been asked to stop. A race window rather
/// than a clock: nothing under test reads it.
const STALL_STEP: Duration = Duration::from_millis(5);

/// The number a proposal comes back under. Any number would do; a test reads it to tell the answer
/// apart from one a test stated itself.
const CREATED_NUMBER: u64 = 7;

/// Where a proposal this fake accepted can be found — the answer a real forge gives, and what a
/// test asserts a create came back with.
pub fn created_url() -> String {
    format!("https://forge.example/pull/{CREATED_NUMBER}")
}

struct Answers {
    readiness: ForgeReadiness,
    base: Mutex<Option<String>>,
    methods: Mutex<Vec<MergeMethod>>,
    templates: Mutex<Vec<PullRequestTemplate>>,
    existing: Mutex<Option<PullRequest>>,
    review: Mutex<Option<PullRequestReview>>,
    log: Mutex<Option<String>>,
    refusal: Mutex<Option<ForgeError>>,
    created: Mutex<Vec<NewPullRequest>>,
    heads: Mutex<Vec<String>>,
    merged: Mutex<Vec<(u64, MergeMethod)>>,
    remarks: Mutex<Vec<String>>,
    log_limits: Mutex<Vec<usize>>,
    review_limits: Mutex<Vec<ReviewLimits>>,
    asks: AtomicUsize,
    inside: AtomicUsize,
    peak: AtomicUsize,
    stalls: AtomicBool,
    delay: Mutex<Duration>,
}

/// An in-memory [`GitForge`]: it reports the readiness it was built with, answers reads from what
/// a test put on it, and turns a proposal into a pull request rather than sending one anywhere.
#[derive(Clone)]
pub struct FakeGitForge {
    answers: Arc<Answers>,
}

impl FakeGitForge {
    /// A forge that can be reached: nothing on offer and nothing proposed yet.
    pub fn ready() -> Self {
        Self::at(ForgeReadiness::Ready)
    }

    /// A forge in a state that can answer nothing — the tool absent, or signed in to no account.
    pub fn at(readiness: ForgeReadiness) -> Self {
        Self {
            answers: Arc::new(Answers {
                readiness,
                base: Mutex::new(None),
                methods: Mutex::new(Vec::new()),
                templates: Mutex::new(Vec::new()),
                existing: Mutex::new(None),
                review: Mutex::new(None),
                log: Mutex::new(None),
                refusal: Mutex::new(None),
                created: Mutex::new(Vec::new()),
                heads: Mutex::new(Vec::new()),
                merged: Mutex::new(Vec::new()),
                remarks: Mutex::new(Vec::new()),
                log_limits: Mutex::new(Vec::new()),
                review_limits: Mutex::new(Vec::new()),
                asks: AtomicUsize::new(0),
                inside: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                stalls: AtomicBool::new(false),
                delay: Mutex::new(Duration::ZERO),
            }),
        }
    }

    /// The same forge, answering only after `delay` — which widens the window in which a second
    /// caller could overlap the first, so a test can prove requests against one repository are
    /// serialized rather than merely usually sequential.
    pub fn slow(self, delay: Duration) -> Self {
        *lock(&self.answers.delay) = delay;
        self
    }

    /// The same forge, whose proposal never finishes on its own — it waits to be asked to stop,
    /// which is what a service that accepts a connection and then says nothing looks like.
    pub fn stalling(self) -> Self {
        self.answers.stalls.store(true, Ordering::SeqCst);
        self
    }

    /// The most requests that were ever inside the port at the same time.
    pub fn peak_concurrent(&self) -> usize {
        self.answers.peak.load(Ordering::SeqCst)
    }

    /// The same forge, whose repository merges into `base` unless told otherwise.
    pub fn merging_into(self, base: &str) -> Self {
        *lock(&self.answers.base) = Some(base.to_string());
        self
    }

    /// The same forge, whose repository carries `templates` as its own convention.
    pub fn carrying(self, templates: Vec<PullRequestTemplate>) -> Self {
        *lock(&self.answers.templates) = templates;
        self
    }

    /// The same forge, whose repository allows `methods` to put a pull request into its base.
    pub fn allowing(self, methods: Vec<MergeMethod>) -> Self {
        *lock(&self.answers.methods) = methods;
        self
    }

    /// The same forge, where the checked-out branch already has `existing` open on it.
    pub fn holding(self, existing: PullRequest) -> Self {
        *lock(&self.answers.existing) = Some(existing);
        self
    }

    /// The same forge, where the checked-out branch's pull request reads back as `review`.
    pub fn reviewing(self, review: PullRequestReview) -> Self {
        *lock(&self.answers.review) = Some(review);
        self
    }

    /// The same forge, where every check's output is `log` — `None` being a check whose output
    /// nothing here can reach, which is an ordinary answer.
    pub fn logging(self, log: Option<&str>) -> Self {
        *lock(&self.answers.log) = log.map(str::to_string);
        self
    }

    /// The same forge, which says each of `remarks` while it merges — what a real one's tool writes
    /// about itself as it works.
    pub fn saying(self, remarks: &[&str]) -> Self {
        *lock(&self.answers.remarks) = remarks.iter().map(|said| said.to_string()).collect();
        self
    }

    /// Every merge the port was asked for, in order.
    pub fn merged(&self) -> Vec<(u64, MergeMethod)> {
        lock(&self.answers.merged).clone()
    }

    /// The ceiling each log request was made under, in order.
    pub fn log_limits(&self) -> Vec<usize> {
        lock(&self.answers.log_limits).clone()
    }

    /// The ceiling each review request was made under, in order.
    pub fn review_limits(&self) -> Vec<ReviewLimits> {
        lock(&self.answers.review_limits).clone()
    }

    /// The same forge, refusing every request with `refusal`.
    pub fn refusing(self, refusal: ForgeError) -> Self {
        *lock(&self.answers.refusal) = Some(refusal);
        self
    }

    /// Every proposal the port was handed, in order — empty when a caller was refused before it
    /// ever got here, which is what a gate holding looks like from outside.
    pub fn created(&self) -> Vec<NewPullRequest> {
        lock(&self.answers.created).clone()
    }

    /// The branch each proposal named, in order.
    pub fn heads(&self) -> Vec<String> {
        lock(&self.answers.heads).clone()
    }

    /// How many requests the port has been asked to make, readiness aside — readiness costs
    /// nothing to answer, so counting it would say a surface reached the service when it did not.
    pub fn asks(&self) -> usize {
        self.answers.asks.load(Ordering::SeqCst)
    }

    /// Counts one request, runs it the way a real one takes time, and refuses it if the forge was
    /// told to refuse. The delay is a race window, not a clock — nothing under test reads it.
    fn asked<T>(&self, answer: impl FnOnce() -> Result<T, ForgeError>) -> Result<T, ForgeError> {
        self.answers.asks.fetch_add(1, Ordering::SeqCst);
        let inside = self.answers.inside.fetch_add(1, Ordering::SeqCst) + 1;
        self.answers.peak.fetch_max(inside, Ordering::SeqCst);
        let delay = *lock(&self.answers.delay);
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        let asked = match lock(&self.answers.refusal).clone() {
            Some(refusal) => Err(refusal),
            None => answer(),
        };
        self.answers.inside.fetch_sub(1, Ordering::SeqCst);
        asked
    }
}

impl GitForge for FakeGitForge {
    fn readiness(&self, _root: &Path) -> ForgeReadiness {
        self.answers.readiness
    }

    fn repository(&self, _root: &Path) -> Result<ForgeRepository, ForgeError> {
        self.asked(|| {
            Ok(ForgeRepository {
                default_base: lock(&self.answers.base).clone(),
                merge_methods: lock(&self.answers.methods).clone(),
            })
        })
    }

    fn templates(&self, _root: &Path) -> Result<Vec<PullRequestTemplate>, ForgeError> {
        self.asked(|| Ok(lock(&self.answers.templates).clone()))
    }

    fn pull_request(&self, _root: &Path, _branch: &str) -> Result<Option<PullRequest>, ForgeError> {
        self.asked(|| Ok(lock(&self.answers.existing).clone()))
    }

    fn create(
        &self,
        _root: &Path,
        branch: &str,
        new: &NewPullRequest,
        stop: &Stop,
    ) -> Result<String, ForgeError> {
        // A real proposal waits on another machine, so a test about stopping one needs a fake that
        // waits too — and the only thing worth waiting for here is being asked to stop.
        if self.answers.stalls.load(Ordering::SeqCst) {
            while !stop.stopped() {
                std::thread::sleep(STALL_STEP);
            }
            return Err(ForgeError::Stopped);
        }
        self.asked(|| {
            lock(&self.answers.created).push(new.clone());
            lock(&self.answers.heads).push(branch.to_string());
            Ok(created_url())
        })
    }

    fn review(
        &self,
        _root: &Path,
        _branch: &str,
        limits: ReviewLimits,
    ) -> Result<Option<PullRequestReview>, ForgeError> {
        self.asked(|| {
            lock(&self.answers.review_limits).push(limits);
            Ok(lock(&self.answers.review).clone())
        })
    }

    fn merge(
        &self,
        _root: &Path,
        number: u64,
        method: MergeMethod,
        _stop: &Stop,
        progress: &Progress,
    ) -> Result<(), ForgeError> {
        self.asked(|| {
            lock(&self.answers.merged).push((number, method));
            for remark in lock(&self.answers.remarks).iter() {
                progress.report(remark);
            }
            Ok(())
        })
    }

    fn check_log(
        &self,
        _root: &Path,
        _check: &CheckRun,
        limit: usize,
    ) -> Result<Option<String>, ForgeError> {
        self.asked(|| {
            lock(&self.answers.log_limits).push(limit);
            Ok(lock(&self.answers.log)
                .clone()
                .map(|log| log.chars().rev().take(limit).collect::<String>())
                .map(|tail| tail.chars().rev().collect()))
        })
    }
}
