//! A [`GitRepository`] fake for the git context's tests: it answers from a queue without
//! running anything, records how many reads it was asked for, tracks the most reads that were
//! ever inside it at once — the observation that proves reads against one repository are
//! serialized — and keeps every change it was asked to make, which is the only trace a change
//! leaves when nothing underneath it is real.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::git::{
    BranchOp, Exchange, GitError, GitRepository, GitStatus, LogRange, RawFileDiff, StashOp,
};
use crate::sync::lock;
use crate::testing::GitChange;
use crate::vcs::{Branches, CommitEntry, DiffTarget, FileContent, HunkRange, ProjectFile};

/// How often a stalled exchange looks at whether it has been asked to stop. A race window rather
/// than a clock: nothing under test reads it.
const STALL_STEP: Duration = Duration::from_millis(5);

struct Answers {
    queued: Mutex<VecDeque<Result<GitStatus, GitError>>>,
    listing: Mutex<Result<Vec<ProjectFile>, GitError>>,
    diff: Mutex<Result<RawFileDiff, GitError>>,
    content: Mutex<Result<Option<FileContent>, GitError>>,
    history: Mutex<Result<Vec<CommitEntry>, GitError>>,
    proposed: Mutex<Result<Vec<CommitEntry>, GitError>>,
    branches: Mutex<Result<Branches, GitError>>,
    stalls: AtomicBool,
    refusal: Mutex<Option<GitError>>,
    changes: Mutex<Vec<GitChange>>,
    reads: AtomicUsize,
    inside: AtomicUsize,
    peak: AtomicUsize,
    delay: Duration,
}

/// An in-memory [`GitRepository`]: each read takes the next queued answer, and the last one
/// repeats once the queue runs out, so a test states only the answers it cares about.
#[derive(Clone)]
pub struct FakeGitRepository {
    answers: Arc<Answers>,
}

impl FakeGitRepository {
    /// A repository that reports `status` for every read.
    pub fn reporting(status: GitStatus) -> Self {
        Self::answering(vec![Ok(status)])
    }

    /// A repository that gives `answers` in order, repeating the last once they run out.
    pub fn answering(answers: Vec<Result<GitStatus, GitError>>) -> Self {
        Self::with_delay(answers, Duration::ZERO)
    }

    /// A repository that reports `status` only after `delay`, widening the window in which a
    /// second read could overlap the first — so a test can prove reads against one repository
    /// are serialized rather than merely usually sequential. The delay is a race window, not a
    /// clock: nothing under test reads it.
    pub fn slow(status: GitStatus, delay: Duration) -> Self {
        Self::with_delay(vec![Ok(status)], delay)
    }

    /// The same repository, listing `files` for every file read. Without this a listing reads
    /// as a folder under no version control, matching the status side's default.
    pub fn listing(self, files: Vec<ProjectFile>) -> Self {
        *lock(&self.answers.listing) = Ok(files);
        self
    }

    /// The same repository, answering `diff` for every diff read. Without this a diff reads as
    /// a folder under no version control, matching the status side's default.
    pub fn diffing(self, diff: RawFileDiff) -> Self {
        *lock(&self.answers.diff) = Ok(diff);
        self
    }

    /// The same repository, holding `content` at every path it is asked to read.
    pub fn holding(self, content: FileContent) -> Self {
        *lock(&self.answers.content) = Ok(Some(content));
        self
    }

    /// The same repository, whose history is `commits`, newest first. Without this a history read
    /// reads as a folder under no version control, matching the status side's default; an explicitly
    /// empty list is the different, ordinary state of a repository with no commits yet.
    pub fn logging(self, commits: Vec<CommitEntry>) -> Self {
        *lock(&self.answers.history) = Ok(commits);
        self
    }

    /// The same repository, where what the checked-out branch holds beyond another branch is
    /// `commits`. Distinct from its whole history, since that is the distinction the range exists
    /// to make; an explicitly empty list is a branch that proposes nothing.
    pub fn proposing(self, commits: Vec<CommitEntry>) -> Self {
        *lock(&self.answers.proposed) = Ok(commits);
        self
    }

    /// The same repository, offering `branches` for every branch read. Without this a branch read
    /// reads as a folder under no version control, matching the status side's default.
    pub fn branching(self, branches: Branches) -> Self {
        *lock(&self.answers.branches) = Ok(branches);
        self
    }

    /// The same repository, whose exchange with a remote never finishes on its own — it waits to be
    /// asked to stop, which is what a remote that accepts a connection and then says nothing looks
    /// like from here.
    pub fn stalling(self) -> Self {
        self.answers.stalls.store(true, Ordering::SeqCst);
        self
    }

    /// The same repository, refusing every change with `refusal` — how a test states that
    /// version control itself said no.
    pub fn refusing(self, refusal: GitError) -> Self {
        *lock(&self.answers.refusal) = Some(refusal);
        self
    }

    /// How many reads the port has been asked for.
    pub fn reads(&self) -> usize {
        self.answers.reads.load(Ordering::SeqCst)
    }

    /// Every change the port was asked to make, in order — empty when a caller was refused
    /// before it ever got here, which is what a gate holding looks like from outside.
    pub fn changes(&self) -> Vec<GitChange> {
        lock(&self.answers.changes).clone()
    }

    /// Files one change and answers as the repository was told to. It runs inside the same
    /// window a read does, so a change overlapping a read against one repository is observable
    /// — which is what makes the per-project gate testable rather than assumed.
    fn changed(&self, change: GitChange) -> Result<(), GitError> {
        self.inside(|| match lock(&self.answers.refusal).clone() {
            Some(refusal) => Err(refusal),
            None => {
                lock(&self.answers.changes).push(change);
                Ok(())
            }
        })
    }

    /// The most reads that were ever inside the port at the same time.
    pub fn peak_concurrent(&self) -> usize {
        self.answers.peak.load(Ordering::SeqCst)
    }

    /// Runs one answer the way a real read behaves: counted, and inside the window the peak
    /// concurrency is measured over. Every read of the port goes through it, so the gate that
    /// keeps one repository to one caller at a time is observable across all of them rather
    /// than only the first.
    fn recorded<T>(&self, answer: impl FnOnce() -> T) -> T {
        self.answers.reads.fetch_add(1, Ordering::SeqCst);
        self.inside(answer)
    }

    /// Runs `act` inside the port the way a real invocation takes time: delayed, and counted
    /// towards the most callers that were ever in there together. The delay is a race window,
    /// not a clock — nothing under test reads it.
    fn inside<T>(&self, act: impl FnOnce() -> T) -> T {
        let inside = self.answers.inside.fetch_add(1, Ordering::SeqCst) + 1;
        self.answers.peak.fetch_max(inside, Ordering::SeqCst);
        if !self.answers.delay.is_zero() {
            std::thread::sleep(self.answers.delay);
        }
        let acted = act();
        self.answers.inside.fetch_sub(1, Ordering::SeqCst);
        acted
    }

    fn with_delay(answers: Vec<Result<GitStatus, GitError>>, delay: Duration) -> Self {
        Self {
            answers: Arc::new(Answers {
                queued: Mutex::new(answers.into()),
                listing: Mutex::new(Err(GitError::NotARepo)),
                diff: Mutex::new(Err(GitError::NotARepo)),
                content: Mutex::new(Err(GitError::NotARepo)),
                history: Mutex::new(Err(GitError::NotARepo)),
                proposed: Mutex::new(Err(GitError::NotARepo)),
                branches: Mutex::new(Err(GitError::NotARepo)),
                stalls: AtomicBool::new(false),
                refusal: Mutex::new(None),
                changes: Mutex::new(Vec::new()),
                reads: AtomicUsize::new(0),
                inside: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                delay,
            }),
        }
    }
}

impl GitRepository for FakeGitRepository {
    fn status(&self, _root: &Path) -> Result<GitStatus, GitError> {
        self.recorded(|| {
            let mut queued = lock(&self.answers.queued);
            let answer = if queued.len() > 1 {
                queued.pop_front()
            } else {
                queued.front().cloned()
            };
            answer.unwrap_or(Err(GitError::NotARepo))
        })
    }

    fn list_files(&self, _root: &Path) -> Result<Vec<ProjectFile>, GitError> {
        self.recorded(|| lock(&self.answers.listing).clone())
    }

    fn diff(
        &self,
        _root: &Path,
        _target: DiffTarget,
        _path: &str,
        _original_path: Option<&str>,
    ) -> Result<RawFileDiff, GitError> {
        self.recorded(|| lock(&self.answers.diff).clone())
    }

    fn read_file(&self, _root: &Path, _path: &str) -> Result<Option<FileContent>, GitError> {
        self.recorded(|| lock(&self.answers.content).clone())
    }

    fn log(
        &self,
        _root: &Path,
        range: LogRange<'_>,
        skip: usize,
        limit: usize,
    ) -> Result<Vec<CommitEntry>, GitError> {
        // Paged for real, so a caller asking for one page of a longer history is exercised rather
        // than handed the whole of it. The range is answered separately, because what a branch
        // holds beyond another branch is a different list from its whole history — a fake that
        // returned one for both would let a caller asking the wrong question look right.
        self.recorded(|| {
            let answers = match range {
                LogRange::CheckedOut => lock(&self.answers.history).clone(),
                LogRange::Since { .. } => lock(&self.answers.proposed).clone(),
            };
            answers.map(|commits| commits.into_iter().skip(skip).take(limit).collect())
        })
    }

    fn stage(&self, _root: &Path, path: &str, original_path: Option<&str>) -> Result<(), GitError> {
        self.changed(GitChange::Stage {
            path: path.to_string(),
            original_path: original_path.map(str::to_string),
        })
    }

    fn unstage(
        &self,
        _root: &Path,
        path: &str,
        original_path: Option<&str>,
    ) -> Result<(), GitError> {
        self.changed(GitChange::Unstage {
            path: path.to_string(),
            original_path: original_path.map(str::to_string),
        })
    }

    fn discard(&self, _root: &Path, path: &str) -> Result<(), GitError> {
        self.changed(GitChange::Discard {
            path: path.to_string(),
        })
    }

    fn stage_hunk(
        &self,
        _root: &Path,
        path: &str,
        _original_path: Option<&str>,
        hunk: HunkRange,
    ) -> Result<(), GitError> {
        self.changed(GitChange::StageHunk {
            path: path.to_string(),
            hunk,
        })
    }

    fn unstage_hunk(
        &self,
        _root: &Path,
        path: &str,
        _original_path: Option<&str>,
        hunk: HunkRange,
    ) -> Result<(), GitError> {
        self.changed(GitChange::UnstageHunk {
            path: path.to_string(),
            hunk,
        })
    }

    fn discard_hunk(&self, _root: &Path, path: &str, hunk: HunkRange) -> Result<(), GitError> {
        self.changed(GitChange::DiscardHunk {
            path: path.to_string(),
            hunk,
        })
    }

    fn commit(&self, _root: &Path, message: &str, amend: bool) -> Result<(), GitError> {
        self.changed(GitChange::Commit {
            message: message.to_string(),
            amend,
        })
    }

    fn branches(&self, _root: &Path, limit: usize) -> Result<Branches, GitError> {
        // Bounded for real, so a caller asking for one page of a longer list is exercised rather
        // than handed the whole of it.
        self.recorded(|| {
            lock(&self.answers.branches)
                .clone()
                .map(|branches| Branches {
                    entries: branches.entries.into_iter().take(limit).collect(),
                    ..branches
                })
        })
    }

    fn branch(&self, _root: &Path, op: BranchOp, name: &str) -> Result<(), GitError> {
        self.changed(GitChange::Branch {
            op,
            name: name.to_string(),
        })
    }

    fn stash(&self, _root: &Path, op: StashOp) -> Result<(), GitError> {
        self.changed(GitChange::Stash { op })
    }

    fn sync(&self, _root: &Path, exchange: Exchange<'_>) -> Result<(), GitError> {
        // A real exchange waits on another machine, so a test about stopping one needs a fake that
        // waits too — and the only thing worth waiting for here is being asked to stop.
        if self.answers.stalls.load(Ordering::SeqCst) {
            while !exchange.stop.stopped() {
                std::thread::sleep(STALL_STEP);
            }
            return Err(GitError::Stopped);
        }
        self.changed(GitChange::Sync {
            op: exchange.op,
            prompting: exchange.prompting,
        })
    }

    fn abort_merge(&self, _root: &Path) -> Result<(), GitError> {
        self.changed(GitChange::AbortMerge)
    }
}
