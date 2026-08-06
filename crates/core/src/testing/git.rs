//! A [`GitRepository`] fake for the git context's tests: it answers from a queue without
//! running anything, records how many reads it was asked for, and tracks the most reads that
//! were ever inside it at once — the observation that proves reads against one repository are
//! serialized.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::git::{Git, GitError, GitRepository, GitStatus, RawFileDiff};
use crate::sync::lock;
use crate::vcs::{
    BranchInfo, ChangeKind, DiffTarget, FileChange, FileContent, GitFileStatus, ProjectFile,
    SyncState,
};

struct Answers {
    queued: Mutex<VecDeque<Result<GitStatus, GitError>>>,
    listing: Mutex<Result<Vec<ProjectFile>, GitError>>,
    diff: Mutex<Result<RawFileDiff, GitError>>,
    content: Mutex<Result<Option<FileContent>, GitError>>,
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

    /// How many reads the port has been asked for.
    pub fn reads(&self) -> usize {
        self.answers.reads.load(Ordering::SeqCst)
    }

    /// The most reads that were ever inside the port at the same time.
    pub fn peak_concurrent(&self) -> usize {
        self.answers.peak.load(Ordering::SeqCst)
    }

    /// Runs one answer the way a real read behaves: counted, delayed, and inside the window the
    /// peak concurrency is measured over. Every call of the port goes through it, so the gate
    /// that keeps one repository to one read at a time is observable across all of them rather
    /// than only the first.
    fn recorded<T>(&self, answer: impl FnOnce() -> T) -> T {
        self.answers.reads.fetch_add(1, Ordering::SeqCst);
        let inside = self.answers.inside.fetch_add(1, Ordering::SeqCst) + 1;
        self.answers.peak.fetch_max(inside, Ordering::SeqCst);
        if !self.answers.delay.is_zero() {
            std::thread::sleep(self.answers.delay);
        }
        let answer = answer();
        self.answers.inside.fetch_sub(1, Ordering::SeqCst);
        answer
    }

    fn with_delay(answers: Vec<Result<GitStatus, GitError>>, delay: Duration) -> Self {
        Self {
            answers: Arc::new(Answers {
                queued: Mutex::new(answers.into()),
                listing: Mutex::new(Err(GitError::NotARepo)),
                diff: Mutex::new(Err(GitError::NotARepo)),
                content: Mutex::new(Err(GitError::NotARepo)),
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

/// One path's diff as the port would produce it, as a test states it.
pub fn raw_diff(header: &str, hunks: &[&str]) -> RawFileDiff {
    RawFileDiff {
        binary: false,
        header: header.to_string(),
        hunks: hunks.iter().map(|hunk| hunk.to_string()).collect(),
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

/// The git context over `repository`, shared as the façade and the watch reactor hold it.
pub fn git_over(repository: FakeGitRepository) -> Arc<Git> {
    Arc::new(Git::new(Arc::new(repository)))
}
