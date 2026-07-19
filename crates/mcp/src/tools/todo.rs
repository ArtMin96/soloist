//! Coordination todo tools: the durable, project-scoped work items agents hand off and coordinate
//! around.
//!
//! A todo carries a small document — a title, a free-form Markdown body, and a lifecycle status. The
//! body is unconstrained (a project template can seed a shape, but nothing is enforced); the create
//! and update tools take exactly those fields. Updates are **revision-guarded** (read, then write
//! back the revision you read). A todo cannot be completed while it has unmet **blockers** (the
//! gate). The **lock** signals cooperative intent and auto-releases when the owning process closes.
//! A todo may also carry an optional **scratchpad** association naming the document it was derived
//! from — stated by name, resolved to a durable id in the core, and never required.
//! Todos survive an app restart; scope, ownership, and the gate are all resolved in the core, not
//! here.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{LinkContent, ProjectId, ScratchpadLink, TodoDoc, TodoId, TodoStatus};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{
    TodoArg, TodoBlockerArg, TodoBlockersArg, TodoCommentCreateArg, TodoCommentEditArg,
    TodoCommentRefArg, TodoCreateArg, TodoGetArg, TodoRef, TodoTagArg, TodoTransferArg,
    TodoUpdateArg,
};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = todo_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "List the todos in your effective project as one-line summaries (id, title, status, tags, blocked, lock, the scratchpad each derives from when it has one, and revision). Todos are durable shared work items that survive restarts."
    )]
    pub(crate) async fn todo_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::TodoList).await {
            Ok(IpcResponse::Todos(todos)) => structured(&serde_json::json!({ "todos": todos })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read one todo by its id or a solo:// link to it. Returns its document (title, Markdown body, status), tags, blockers, which blockers are still unmet, comments, lock, the scratchpad it derives from (null when it has none), and the revision — pass that revision back to todo_update to update it safely."
    )]
    pub(crate) async fn todo_get(
        &self,
        Parameters(TodoGetArg { todo }): Parameters<TodoGetArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match todo {
            TodoRef::Id(id) => {
                self.todo_view(IpcRequest::TodoGet {
                    todo: TodoId::from_raw(id),
                })
                .await
            }
            TodoRef::Link(link) => self.read_solo_link(link).await,
        }
    }

    #[tool(
        description = "Create a todo: a title, an optional free-form Markdown body (what needs doing and any detail), and an initial status (defaults to open). Leaving the body empty seeds it from the project's default todo template (if one is selected); the reply's `seeded_from` names it. Set `scratchpad` only when this todo derives from that scratchpad — say, a task you extracted from its plan — so the board can group it under the document it came from; otherwise omit it, since most todos have none. Returns the new todo with its id."
    )]
    pub(crate) async fn todo_create(
        &self,
        Parameters(TodoCreateArg {
            title,
            body,
            status,
            scratchpad,
        }): Parameters<TodoCreateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let doc = TodoDoc {
            title,
            body: body.unwrap_or_default(),
            status: status.map_or(TodoStatus::Open, Into::into),
        };
        match self
            .client
            .request(IpcRequest::TodoCreate { doc, scratchpad })
            .await
        {
            Ok(IpcResponse::TodoCreated { todo, seeded_from }) => {
                structured(&serde_json::json!({ "todo": todo, "seeded_from": seeded_from }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Update a todo, revision-guarded — the whole document is replaced. Provide the title and status you want and the revision you read from todo_get (a mismatch means someone edited it first); the Markdown body is optional. Set status to done only when its blockers are all complete (otherwise use todo_complete, which enforces the gate). The `scratchpad` link is NOT part of the document: omitting it leaves any existing link untouched, a name (re)links the todo to the scratchpad it derives from, and an explicit null unlinks it."
    )]
    pub(crate) async fn todo_update(
        &self,
        Parameters(TodoUpdateArg {
            todo,
            title,
            body,
            status,
            scratchpad,
            expected_revision,
        }): Parameters<TodoUpdateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let doc = TodoDoc {
            title,
            body: body.unwrap_or_default(),
            status: status.into(),
        };
        self.todo_view(IpcRequest::TodoUpdate {
            todo: TodoId::from_raw(todo),
            doc,
            scratchpad: match scratchpad {
                None => ScratchpadLink::Unchanged,
                Some(stated) => ScratchpadLink::stated(stated),
            },
            expected_revision,
        })
        .await
    }

    #[tool(
        description = "Mark a todo done. Refused while it still has unmet blockers — a todo stays gated until its blockers complete."
    )]
    pub(crate) async fn todo_complete(
        &self,
        Parameters(TodoArg { todo }): Parameters<TodoArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoComplete {
            todo: TodoId::from_raw(todo),
        })
        .await
    }

    #[tool(description = "Delete a todo. Returns whether one was removed under `deleted`.")]
    pub(crate) async fn todo_delete(
        &self,
        Parameters(TodoArg { todo }): Parameters<TodoArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TodoDelete {
                todo: TodoId::from_raw(todo),
            })
            .await
        {
            Ok(IpcResponse::TodoDeleted(deleted)) => {
                structured(&serde_json::json!({ "deleted": deleted }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Move a todo to another project, keeping its comments and completion status and clearing its blockers, lock, and scratchpad link (those all reference the source project). You must be authenticated to the destination — a process you run in belongs to it. Note: an MCP session is scoped to a single project, so a genuine cross-project move over MCP is refused; the desktop app performs cross-project transfers."
    )]
    pub(crate) async fn todo_transfer(
        &self,
        Parameters(TodoTransferArg { todo, to_project }): Parameters<TodoTransferArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TodoTransfer {
                todo: TodoId::from_raw(todo),
                to_project: ProjectId::from_raw(to_project),
            })
            .await
        {
            Ok(IpcResponse::Todo(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "List the distinct tags used across your effective project's todos.")]
    pub(crate) async fn todo_tags_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::TodoTagsList).await {
            Ok(IpcResponse::TodoTags(tags)) => structured(&serde_json::json!({ "tags": tags })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Add a tag to a todo (idempotent). Returns the updated todo.")]
    pub(crate) async fn todo_add_tag(
        &self,
        Parameters(TodoTagArg { todo, tag }): Parameters<TodoTagArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoAddTag {
            todo: TodoId::from_raw(todo),
            tag,
        })
        .await
    }

    #[tool(description = "Remove a tag from a todo. Returns the updated todo.")]
    pub(crate) async fn todo_remove_tag(
        &self,
        Parameters(TodoTagArg { todo, tag }): Parameters<TodoTagArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoRemoveTag {
            todo: TodoId::from_raw(todo),
            tag,
        })
        .await
    }

    #[tool(
        description = "Replace a todo's blockers — the todos that must complete before it can. Each must exist and not be the todo itself. Returns the updated todo."
    )]
    pub(crate) async fn todo_set_blockers(
        &self,
        Parameters(TodoBlockersArg { todo, blockers }): Parameters<TodoBlockersArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoSetBlockers {
            todo: TodoId::from_raw(todo),
            blockers: blockers.into_iter().map(TodoId::from_raw).collect(),
        })
        .await
    }

    #[tool(description = "Add one blocker to a todo (idempotent). Returns the updated todo.")]
    pub(crate) async fn todo_add_blocker(
        &self,
        Parameters(TodoBlockerArg { todo, blocker }): Parameters<TodoBlockerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoAddBlocker {
            todo: TodoId::from_raw(todo),
            blocker: TodoId::from_raw(blocker),
        })
        .await
    }

    #[tool(description = "Remove one blocker from a todo. Returns the updated todo.")]
    pub(crate) async fn todo_remove_blocker(
        &self,
        Parameters(TodoBlockerArg { todo, blocker }): Parameters<TodoBlockerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoRemoveBlocker {
            todo: TodoId::from_raw(todo),
            blocker: TodoId::from_raw(blocker),
        })
        .await
    }

    #[tool(
        description = "Lock a todo to signal you are working on it (signals, not ownership — it does not block others). The returned todo's `locked_by` reports the holder; the lock auto-releases when your process closes. Needs a bound process."
    )]
    pub(crate) async fn todo_lock(
        &self,
        Parameters(TodoArg { todo }): Parameters<TodoArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoLock {
            todo: TodoId::from_raw(todo),
        })
        .await
    }

    #[tool(description = "Release your lock on a todo. Returns the updated todo.")]
    pub(crate) async fn todo_unlock(
        &self,
        Parameters(TodoArg { todo }): Parameters<TodoArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoUnlock {
            todo: TodoId::from_raw(todo),
        })
        .await
    }

    #[tool(
        description = "Add a comment to a todo. Returns the updated todo and the new comment's id under `comment`."
    )]
    pub(crate) async fn todo_comment_create(
        &self,
        Parameters(TodoCommentCreateArg { todo, body }): Parameters<TodoCommentCreateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TodoCommentCreate {
                todo: TodoId::from_raw(todo),
                body,
            })
            .await
        {
            Ok(IpcResponse::TodoComment { todo, comment }) => {
                structured(&serde_json::json!({ "todo": todo, "comment": comment }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Update a comment on a todo. Returns the updated todo.")]
    pub(crate) async fn todo_comment_update(
        &self,
        Parameters(TodoCommentEditArg {
            todo,
            comment,
            body,
        }): Parameters<TodoCommentEditArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoCommentUpdate {
            todo: TodoId::from_raw(todo),
            comment,
            body,
        })
        .await
    }

    #[tool(description = "Delete a comment from a todo. Returns the updated todo.")]
    pub(crate) async fn todo_comment_delete(
        &self,
        Parameters(TodoCommentRefArg { todo, comment }): Parameters<TodoCommentRefArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.todo_view(IpcRequest::TodoCommentDelete {
            todo: TodoId::from_raw(todo),
            comment,
        })
        .await
    }

    #[tool(description = "List the comments on a todo.")]
    pub(crate) async fn todo_comment_list(
        &self,
        Parameters(TodoArg { todo }): Parameters<TodoArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TodoCommentList {
                todo: TodoId::from_raw(todo),
            })
            .await
        {
            Ok(IpcResponse::TodoComments(comments)) => {
                structured(&serde_json::json!({ "comments": comments }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}

impl SoloistMcp {
    /// Sends a request that returns a single todo and projects the view — the common shape of the
    /// todo tools, single-sourced so every one renders the todo identically.
    async fn todo_view(&self, request: IpcRequest) -> Result<CallToolResult, ErrorData> {
        match self.client.request(request).await {
            Ok(IpcResponse::Todo(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    /// Resolves a `solo://` link to its content and renders the scratchpad or todo it points to —
    /// shared by `scratchpad_read` and `todo_get` so a copied link reads the same from either, and
    /// scope is enforced in the core (a foreign-scope or malformed link comes back as a tool error).
    pub(crate) async fn read_solo_link(&self, link: String) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ResolveLink { link }).await {
            Ok(IpcResponse::Link(LinkContent::Scratchpad(view))) => structured(&view),
            Ok(IpcResponse::Link(LinkContent::Todo(view))) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
