//! A [`GitRepository`] fake for the git context's tests: it answers from a queue without
//! running anything, records how many reads it was asked for, tracks the most reads that were
//! ever inside it at once — the observation that proves reads against one repository are
//! serialized — and keeps every change it was asked to make, which is the only trace a change
//! leaves when nothing underneath it is real.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::git::{Git, GitError, GitRepository, GitStatus, RawFileDiff, RawHunk};
use crate::ids::ProjectId;
use crate::ports::TrustRepo;
use crate::sync::lock;
use crate::testing::FakeTrustRepo;
use crate::vcs::{
    BranchInfo, ChangeKind, DiffTarget, FileChange, FileContent, GitFileStatus, HunkRange,
    ProjectFile, SyncState,
};

/// One change a working tree was asked to undergo, as the port received it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GitChange {
    Stage {
        path: String,
        original_path: Option<String>,
    },
    Unstage {
        path: String,
        original_path: Option<String>,
    },
    Discard {
        path: String,
    },
    StageHunk {
        path: String,
        hunk: HunkRange,
    },
    UnstageHunk {
        path: String,
        hunk: HunkRange,
    },
    DiscardHunk {
        path: String,
        hunk: HunkRange,
    },
    Commit {
        message: String,
        amend: bool,
    },
}

struct Answers {
    queued: Mutex<VecDeque<Result<GitStatus, GitError>>>,
    listing: Mutex<Result<Vec<ProjectFile>, GitError>>,
    diff: Mutex<Result<RawFileDiff, GitError>>,
    content: Mutex<Result<Option<FileContent>, GitError>>,
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
}

/// One entry in a project's file listing, as a test states it.
pub fn project_file(path: &str, ignored: bool) -> ProjectFile {
    ProjectFile {
        path: path.to_string(),
        ignored,
    }
}

/// One changed path, as a test states it.
pub fn file_change(
    path: &str,
    staged: Option<ChangeKind>,
    unstaged: Option<ChangeKind>,
) -> FileChange {
    FileChange {
        path: path.to_string(),
        status: GitFileStatus { staged, unstaged },
        original_path: None,
    }
}

/// One path's diff as the port would produce it, as a test states it. The hunks are given their
/// ranges in order from line one, which is enough for anything that only needs them to differ.
pub fn raw_diff(header: &str, hunks: &[&str]) -> RawFileDiff {
    RawFileDiff {
        binary: false,
        header: header.to_string(),
        hunks: hunks
            .iter()
            .enumerate()
            .map(|(index, hunk)| RawHunk {
                range: hunk_range(index as u32 + 1),
                text: hunk.to_string(),
            })
            .collect(),
    }
}

/// A hunk covering one line at `line` on both sides, as a test states it.
pub fn hunk_range(line: u32) -> HunkRange {
    HunkRange {
        old_start: line,
        old_lines: 1,
        new_start: line,
        new_lines: 1,
    }
}

/// A clean working tree on `branch`, tracking nothing — the starting point a test varies.
pub fn git_status(branch: &str) -> GitStatus {
    GitStatus {
        branch: BranchInfo {
            name: Some(branch.to_string()),
            upstream: None,
            sync: SyncState::Unknown,
        },
        changes: Vec::new(),
    }
}

/// A trust record with nothing trusted — the state every project starts in, and all a read
/// needs, since reading a working tree is ungated.
pub fn untrusting() -> Arc<dyn TrustRepo> {
    Arc::new(FakeTrustRepo::new())
}

/// The git context over `repository`, shared as the façade and the watch reactor hold it, with
/// no project trusted to be changed — so a read behaves as it always does and a change is
/// refused, which is the state a project starts in.
pub fn git_over(repository: FakeGitRepository) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(FakeTrustRepo::new()),
    ))
}

/// The same, with `project` already trusted to be changed — the starting point for a test about
/// what a change does rather than about whether it is allowed.
pub fn git_trusting(repository: FakeGitRepository, project: ProjectId) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
    ))
}
