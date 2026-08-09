//! The working-tree values a git test states its case in, and the context it states them to.
//!
//! Separate from the fake that answers with them ([`FakeGitRepository`](super::FakeGitRepository))
//! because they are the opposite half of a test: one is a stand-in for the port, these are the
//! vocabulary a test writes its expectations in. Every one of them is the shortest way to say one
//! thing, so a test reads as what it is about rather than as struct construction.

use std::sync::Arc;

use crate::git::{
    CheckRun, CheckState, Git, GitStatus, NoopFileOpener, NoopGitForge, PullRequest,
    PullRequestReview, PullRequestState, PullRequestTemplate, RawFileDiff, RawHunk, ReviewComment,
    ReviewThread,
};
use crate::ids::ProjectId;
use crate::ports::TrustRepo;
use crate::testing::{FakeFileOpener, FakeGitRepository, FakeTrustRepo};
use crate::vcs::{
    Branch, BranchInfo, Branches, ChangeKind, CommitEntry, FileChange, GitFileStatus, HunkRange,
    ProjectFile, SyncState,
};

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

/// One commit saying only its subject, as a test states it. A commit whose message says more is
/// stated with [`described_entry`], and one joining two lines of history with [`merge_entry`].
pub fn commit_entry(id: &str, subject: &str, author: &str) -> CommitEntry {
    CommitEntry {
        id: id.to_string(),
        subject: subject.to_string(),
        body: String::new(),
        author: author.to_string(),
        authored_at: 1_700_000_000,
        merge: false,
    }
}

/// One commit whose message says more than its subject, as a test states it.
pub fn described_entry(id: &str, subject: &str, body: &str) -> CommitEntry {
    CommitEntry {
        body: body.to_string(),
        ..commit_entry(id, subject, "Somebody")
    }
}

/// One commit that joins two lines of history, as a test states it.
pub fn merge_entry(id: &str, subject: &str) -> CommitEntry {
    CommitEntry {
        merge: true,
        ..commit_entry(id, subject, "Somebody")
    }
}

/// A clean working tree on `branch`, tracking nothing and merging nothing — the starting point a
/// test varies.
pub fn git_status(branch: &str) -> GitStatus {
    GitStatus {
        branch: BranchInfo {
            name: Some(branch.to_string()),
            upstream: None,
            sync: SyncState::Unknown,
        },
        changes: Vec::new(),
        merging: false,
    }
}

/// The same working tree, tracking `upstream` and standing level with it.
pub fn tracking_status(branch: &str, upstream: &str) -> GitStatus {
    GitStatus {
        branch: BranchInfo {
            upstream: Some(upstream.to_string()),
            sync: SyncState::UpToDate,
            ..git_status(branch).branch
        },
        ..git_status(branch)
    }
}

/// One branch a switcher could offer, as a test states it.
pub fn branch(name: &str, head: bool) -> Branch {
    Branch {
        name: name.to_string(),
        upstream: None,
        head,
    }
}

/// What a switcher can act on, as a test states it: `entries` and nothing set aside.
pub fn branches(entries: Vec<Branch>) -> Branches {
    Branches {
        entries,
        stashed: false,
    }
}

/// One description skeleton on offer, as a test states it.
pub fn pull_request_template(name: &str, body: &str) -> PullRequestTemplate {
    PullRequestTemplate {
        name: name.to_string(),
        body: body.to_string(),
    }
}

/// One open pull request on `head`, as a test states it.
pub fn pull_request(number: u64, head: &str) -> PullRequest {
    PullRequest {
        number,
        url: format!("https://forge.example/pull/{number}"),
        title: format!("Whatever {head} proposes"),
        state: PullRequestState::Open,
        draft: false,
        base: "main".to_string(),
        head: head.to_string(),
    }
}

/// One check the service ran, in whatever state a test is about.
pub fn check_run(name: &str, state: CheckState) -> CheckRun {
    CheckRun {
        name: name.to_string(),
        state,
        workflow: Some("Tests".to_string()),
        url: Some(format!(
            "https://forge.example/owner/repo/actions/runs/9/job/{}",
            name.len()
        )),
    }
}

/// One conversation hanging on a line of the diff, carrying one comment.
pub fn review_thread(id: &str, path: &str, line: u64, body: &str) -> ReviewThread {
    ReviewThread {
        id: id.to_string(),
        url: Some(format!("https://forge.example/pull/12#{id}")),
        path: Some(path.to_string()),
        line: Some(line),
        resolved: false,
        outdated: false,
        comments: vec![ReviewComment {
            author: "octocat".to_string(),
            body: body.to_string(),
            url: Some(format!("https://forge.example/pull/12#{id}")),
        }],
    }
}

/// What a branch's open pull request reads back as, with whatever checks and conversations a test
/// is about.
pub fn pull_request_review(
    head: &str,
    checks: Vec<CheckRun>,
    threads: Vec<ReviewThread>,
) -> PullRequestReview {
    PullRequestReview {
        pull_request: pull_request(12, head),
        checks,
        threads,
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
///
/// No forge and no desktop: a test about the working tree is not a test about a hosting service
/// or about opening a file, and the no-op ports are what a machine without either behaves as.
pub fn git_over(repository: FakeGitRepository) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        Arc::new(FakeTrustRepo::new()),
    ))
}

/// The same, with `project` already trusted to be changed — the starting point for a test about
/// what a change does rather than about whether it is allowed.
pub fn git_trusting(repository: FakeGitRepository, project: ProjectId) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
    ))
}

/// The git context over `opener`, for a test about handing a file to a desktop rather than about
/// reading a working tree. `trusted` states whether the project may be acted within at all, which
/// is the gate such a test is usually about.
pub fn git_opening(opener: FakeFileOpener, project: ProjectId, trusted: bool) -> Arc<Git> {
    let trust = FakeTrustRepo::new();
    Arc::new(Git::new(
        Arc::new(FakeGitRepository::answering(Vec::new())),
        Arc::new(NoopGitForge),
        Arc::new(opener),
        Arc::new(if trusted {
            trust.trusting_project(project)
        } else {
            trust
        }),
    ))
}
