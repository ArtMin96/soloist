//! Version-control tools: what the project's repository says, what an agent may change about it,
//! and the pull requests it has on the hosting service.
//!
//! This group is **off until the user turns it on**, because its tools change the user's own
//! repository under whatever credentials their `git` reaches for. Once on, every tool acts on the
//! session's effective project and takes no project of its own, so there is nothing to address
//! another project's repository with; the trust the user granted that project is what every change
//! is spent against, and it is spent in the core rather than here.
//!
//! There is no confirmation step. The local user's surface asks before it throws work away because
//! somebody is there to ask; nothing here can, so the trust gate is the whole of the guard and the
//! destructive tools say so in their own descriptions.
//!
//! An exchange with a remote never stops to ask anybody for a credential — a question nobody is
//! waiting at is a wait, not a prompt — so one that needs a person fails promptly instead.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{
    GitBranchArg, GitCommitArg, GitCreatePullRequestArg, GitDiffArg, GitMergePullRequestArg,
    GitPathArg,
};
use crate::client::ClientError;
use crate::server::SoloistMcp;
use crate::tools::progress::Reporting;
use crate::tools::reply::{acked, app_error, structured, unexpected};

#[tool_router(router = git_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Read the working-tree status of this project's repository: the checked-out branch, how it stands against its upstream (ahead/behind), every path that differs from the last commit with its staged and unstaged change, and whether a merge is under way. Start here — the paths it reports are what git_diff, git_stage, git_unstage and git_discard address."
    )]
    pub(crate) async fn git_status(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::GitStatus).await {
            Ok(IpcResponse::GitStatus(status)) => structured(&status),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read one path's unified diff. `target` picks the comparison: staged (what the next commit would record), unstaged (what the working tree holds beyond the index), or head (the working tree against the last commit). Returns the patch, the range of every hunk in it, and whether it was cut short. Returns null when the path names nothing inside the repository."
    )]
    pub(crate) async fn git_diff(
        &self,
        Parameters(GitDiffArg { path, target, full }): Parameters<GitDiffArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GitDiff {
            path,
            target: target.into(),
            extent: extent(full),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::GitDiff(diff)) => structured(&serde_json::json!({ "diff": diff })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "List the branches this project could switch to, most recently committed to first, and whether it has anything set aside in the stash."
    )]
    pub(crate) async fn git_branches(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::GitBranches).await {
            Ok(IpcResponse::GitBranches(branches)) => structured(&branches),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Record what the working tree holds for a path in the index, so the next commit carries it. Pass a hunk from git_diff to stage only that part of the change instead of the whole file."
    )]
    pub(crate) async fn git_stage(
        &self,
        Parameters(GitPathArg { path, hunk }): Parameters<GitPathArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GitStage {
            path,
            hunk: hunk.map(Into::into),
        };
        self.acked_change(request).await
    }

    #[tool(
        description = "Take a path back out of the index, leaving the working tree untouched — the change stays, it just stops being staged. Pass a hunk from git_diff to unstage only that part."
    )]
    pub(crate) async fn git_unstage(
        &self,
        Parameters(GitPathArg { path, hunk }): Parameters<GitPathArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GitUnstage {
            path,
            hunk: hunk.map(Into::into),
        };
        self.acked_change(request).await
    }

    #[tool(
        description = "Throw away what the working tree holds for a path beyond the index, restoring it from the index. Destructive and not undoable, and there is no confirmation step over MCP — the user having trusted this project is the only guard. It cannot reach past the index, so nothing staged or committed can be lost; a path version control does not track is refused rather than deleted. Pass a hunk from git_diff to discard only that part."
    )]
    pub(crate) async fn git_discard(
        &self,
        Parameters(GitPathArg { path, hunk }): Parameters<GitPathArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GitDiscard {
            path,
            hunk: hunk.map(Into::into),
        };
        self.acked_change(request).await
    }

    #[tool(
        description = "Record the index as a commit. The user's own hooks, signing key and configuration all apply, because it is their git that runs — a hook that rejects the commit comes back with what it wrote and nothing is committed. Refused with nothing staged, and refused on a project the user has not trusted."
    )]
    pub(crate) async fn git_commit(
        &self,
        Parameters(GitCommitArg { message, amend }): Parameters<GitCommitArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.acked_change(IpcRequest::GitCommit { message, amend })
            .await
    }

    #[tool(
        description = "Hand the checked-out branch's commits to its remote, publishing the branch when it tracks nothing yet. Runs under the user's own credentials; where a credential needs a person, this fails promptly rather than waiting, since nobody is at the window to answer. The user can stop it from the app, which comes back as the stopped refusal rather than a failure."
    )]
    pub(crate) async fn git_push(&self, reporting: Reporting) -> Result<CallToolResult, ErrorData> {
        self.reported_change(reporting, |progress| IpcRequest::GitPush { progress })
            .await
    }

    #[tool(
        description = "Bring the remote's commits in and reconcile them with what is checked out, however the user's own configuration says to. Where they have not said how, version control refuses rather than guessing, and its refusal is what comes back; a pull that conflicts leaves the conflict in the working tree to be resolved."
    )]
    pub(crate) async fn git_pull(&self, reporting: Reporting) -> Result<CallToolResult, ErrorData> {
        self.reported_change(reporting, |progress| IpcRequest::GitPull { progress })
            .await
    }

    #[tool(
        description = "Bring the remote's commits in without touching the working tree, which is what makes the ahead/behind counts git_status reports true again."
    )]
    pub(crate) async fn git_fetch(
        &self,
        reporting: Reporting,
    ) -> Result<CallToolResult, ErrorData> {
        self.reported_change(reporting, |progress| IpcRequest::GitFetch { progress })
            .await
    }

    #[tool(description = "Start a branch at what is currently checked out and switch to it.")]
    pub(crate) async fn git_create_branch(
        &self,
        Parameters(GitBranchArg { name }): Parameters<GitBranchArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.acked_change(IpcRequest::GitCreateBranch { name })
            .await
    }

    #[tool(
        description = "Check out an existing branch. A switch that would overwrite uncommitted work is refused in version control's own words — nothing is stashed or discarded to get past it, so stash first if that is what you meant."
    )]
    pub(crate) async fn git_switch_branch(
        &self,
        Parameters(GitBranchArg { name }): Parameters<GitBranchArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.acked_change(IpcRequest::GitSwitchBranch { name })
            .await
    }

    #[tool(
        description = "Delete a branch. Destructive, and there is no confirmation step over MCP — the user having trusted this project is the only guard. Never forced: a branch holding commits nothing else holds is refused and stays refused."
    )]
    pub(crate) async fn git_delete_branch(
        &self,
        Parameters(GitBranchArg { name }): Parameters<GitBranchArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.acked_change(IpcRequest::GitDeleteBranch { name })
            .await
    }

    #[tool(
        description = "Set what the working tree holds aside, leaving it as the last commit left it. Take it back with git_pop_stash."
    )]
    pub(crate) async fn git_stash(&self) -> Result<CallToolResult, ErrorData> {
        self.acked_change(IpcRequest::GitStash).await
    }

    #[tool(
        description = "Put the most recently stashed changes back and forget them. A collision with what the working tree holds now comes back as version control's own account of it, with the conflict left in the working tree to resolve."
    )]
    pub(crate) async fn git_pop_stash(&self) -> Result<CallToolResult, ErrorData> {
        self.acked_change(IpcRequest::GitPopStash).await
    }

    #[tool(
        description = "Read what this project can propose as a pull request: whether the GitHub tool is installed and signed in, the branch that would be proposed and the branch it would merge into, the pull request this branch already has, the description templates the repository expects, and the merge methods it permits. Call this before git_create_pull_request — it reports the base branch and the shape the description should take."
    )]
    pub(crate) async fn git_pull_request(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::GitPullRequest).await {
            Ok(IpcResponse::GitPullRequest(surface)) => structured(&surface),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read the open pull request for the checked-out branch under review: the pull request itself, what each of the service's checks concluded, and every conversation people have had on it with where in the diff it hangs. Returns null when the branch has nothing open. Both the conversation list and each conversation's comments are capped."
    )]
    pub(crate) async fn git_pull_request_review(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::GitPullRequestReview).await {
            Ok(IpcResponse::GitPullRequestReview(review)) => {
                structured(&serde_json::json!({ "review": review }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Propose what is checked out as a pull request, publishing the branch to the remote first if it is not there as it stands. Answers with the address of what was made. The branch proposed is always the checked-out one, read from the repository rather than named here."
    )]
    pub(crate) async fn git_create_pull_request(
        &self,
        Parameters(arg): Parameters<GitCreatePullRequestArg>,
        reporting: Reporting,
    ) -> Result<CallToolResult, ErrorData> {
        let new = arg.into();
        let answered = self
            .reported(reporting, |progress| IpcRequest::GitCreatePullRequest {
                new,
                progress,
            })
            .await;
        match answered {
            Ok(IpcResponse::GitPullRequestCreated(url)) => {
                structured(&serde_json::json!({ "url": url }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Put a pull request's commits into its base branch. Destructive to the base branch and there is no confirmation step over MCP — the user having trusted this project is the only guard. Whether it is allowed at all is the repository's own rule: a required check that has not passed or a review that is owed comes back as the service's refusal, and nothing is merged."
    )]
    pub(crate) async fn git_merge_pull_request(
        &self,
        Parameters(GitMergePullRequestArg { number, method }): Parameters<GitMergePullRequestArg>,
        reporting: Reporting,
    ) -> Result<CallToolResult, ErrorData> {
        let method = method.into();
        self.reported_change(reporting, |progress| IpcRequest::GitMergePullRequest {
            number,
            method,
            progress,
        })
        .await
    }

    /// [`acked_change`](Self::acked_change), for the changes that reach another machine and can
    /// take minutes to do it.
    async fn reported_change(
        &self,
        reporting: Reporting,
        request: impl FnOnce(bool) -> IpcRequest,
    ) -> Result<CallToolResult, ErrorData> {
        match self.reported(reporting, request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    /// One request that says what it is doing while it does it, for whoever asked to be told.
    ///
    /// Whether anything is reported is the caller's choice alone, carried by the progress token it
    /// put on the request: without one this is an ordinary request and what goes to the app is the
    /// one that always went. With one, the app is asked to say what version control is saying while
    /// it says it, and each remark becomes a notification against that token.
    ///
    /// Answers with the app's own reply rather than a result, because what a reported operation
    /// comes back with is its own — an acknowledgement for a change, an address for a proposal.
    async fn reported(
        &self,
        reporting: Reporting,
        request: impl FnOnce(bool) -> IpcRequest,
    ) -> Result<IpcResponse, ClientError> {
        let Some((reports, forwarding)) = reporting.forwarding() else {
            return self.client.request(request(false)).await;
        };
        let answered = self.client.request_reporting(request(true), reports).await;
        // The request owned the only sender, so awaiting here is what makes the last remark reach
        // the caller before the answer does — and what stops the forwarding after it.
        let _ = forwarding.await;
        answered
    }

    /// The reply every change to a repository shares: the app acknowledges, or refuses with the
    /// word that says why. Single-sourced so no change tool invents a different success shape.
    async fn acked_change(&self, request: IpcRequest) -> Result<CallToolResult, ErrorData> {
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}

/// How much of a diff to carry: what the caller asked for, in the core's own vocabulary.
fn extent(full: bool) -> soloist_core::DiffExtent {
    match full {
        true => soloist_core::DiffExtent::Full,
        false => soloist_core::DiffExtent::Capped,
    }
}
