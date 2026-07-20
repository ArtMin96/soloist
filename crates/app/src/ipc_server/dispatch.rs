//! Request routing: the one place the IPC wire meets the core.
//!
//! Every request maps to exactly one [`Facade`] call, so MCP, the UI, and the HTTP API share one
//! behaviour rather than each re-implementing an action. No domain logic lives here — identity,
//! project scope, and the trust gate are all resolved in the core, and a session-scoped request
//! routes through [`ScopedFacade`](soloist_core::ScopedFacade) so it cannot reach an ungated door.
//!
//! The dispatch is one exhaustive `match` over a closed [`IpcRequest`], deliberately: the wire
//! names are already mapped to variants by `serde`, and matching the variants means a new request
//! cannot be added without the compiler demanding it be routed. It reads long because it is wide —
//! one flat arm per request, none interacting — not because it is complex.

use std::sync::Arc;
use std::time::Duration;

use soloist_core::{Facade, IdleMode, ProjectId, RenderRequest, SessionId, WaitForPortError};
use soloist_ipc::{
    IpcError, IpcRequest, IpcResponse, IpcResult, PortWaitOutcome, ProjectStatus, ProjectSummary,
};

/// The port-readiness wait when the caller names no timeout.
const DEFAULT_PORT_WAIT: Duration = Duration::from_secs(10);
/// The longest a `wait_for_bound_port` blocks, regardless of the requested timeout. Kept
/// well under the IPC client's per-request timeout so the wait resolves as a structured
/// "not bound yet" rather than a transport timeout, and a remote caller cannot tie up the
/// connection with a huge value.
const MAX_PORT_WAIT: Duration = Duration::from_secs(25);

/// Routes one request to the single matching [`Facade`] method and projects the result back — the
/// only place the IPC wire meets the core, adding no domain logic of its own (identity, scope, and
/// the trust gate all live in the core).
///
/// The three requests that themselves await the core (`send_input`/`close_process`/a port wait)
/// stay on the runtime; every other request is a **synchronous** core call, so it runs on the
/// blocking pool via [`spawn_blocking`]. A durable-store write's `fsync` can then never park a
/// runtime worker — no blocking call runs on the `tokio` runtime.
///
/// [`spawn_blocking`]: tokio::task::spawn_blocking
pub(super) async fn handle_request(
    facade: &Arc<Facade>,
    session: SessionId,
    request: IpcRequest,
) -> IpcResult {
    match request {
        IpcRequest::CloseProcess { process } => {
            return facade
                .scoped(session)
                .close_process(process)
                .await
                .map(|()| IpcResponse::Acked)
                .map_err(IpcError::from);
        }
        IpcRequest::SendInput {
            process,
            input,
            wait_ms,
        } => {
            return facade
                .scoped(session)
                .send_input(
                    process,
                    input.into_bytes(),
                    wait_ms.map(Duration::from_millis),
                )
                .await
                .map(IpcResponse::InputSent)
                .map_err(IpcError::from);
        }
        IpcRequest::WaitForBoundPort {
            process,
            port,
            timeout_ms,
        } => {
            // Waiting on a port reveals whether the process bound it — the same disclosure the
            // scoped port read refuses, so a cross-project target is refused here too.
            facade
                .scoped(session)
                .require_in_scope(process)
                .map_err(IpcError::from)?;
            let timeout = timeout_ms
                .map_or(DEFAULT_PORT_WAIT, Duration::from_millis)
                .min(MAX_PORT_WAIT);
            let outcome = match facade.wait_for_port(process, port, timeout).await {
                Ok(()) => PortWaitOutcome::Bound,
                Err(WaitForPortError::Timeout) => PortWaitOutcome::TimedOut,
                Err(WaitForPortError::NotRunning) => PortWaitOutcome::NotRunning,
            };
            return Ok(IpcResponse::PortWait(outcome));
        }
        _ => {}
    }
    let facade = Arc::clone(facade);
    tokio::task::spawn_blocking(move || dispatch_blocking(&facade, session, request))
        .await
        .unwrap_or_else(|err| {
            Err(IpcError::Internal(format!(
                "request handler panicked: {err}"
            )))
        })
}

/// The synchronous request dispatch — every request except the three that await the core. Runs on
/// the blocking pool (see [`handle_request`]) so its durable-store calls never block a runtime
/// worker.
fn dispatch_blocking(facade: &Facade, session: SessionId, request: IpcRequest) -> IpcResult {
    match request {
        // Handled on the runtime by `handle_request` before reaching here; a value (not a panic)
        // keeps the connection alive if one ever slipped through.
        IpcRequest::CloseProcess { .. }
        | IpcRequest::SendInput { .. }
        | IpcRequest::WaitForBoundPort { .. } => Err(IpcError::Internal(
            "request must be awaited on the runtime".into(),
        )),
        IpcRequest::Whoami => Ok(IpcResponse::Whoami(facade.scoped(session).whoami())),
        IpcRequest::BindSessionProcess { process } => facade
            .scoped(session)
            .bind_session_process(process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::RegisterAgent { label } => {
            facade.scoped(session).register_agent(label);
            Ok(IpcResponse::Acked)
        }
        IpcRequest::SelectProject { project } => facade
            .scoped(session)
            .select_project(project)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::SelectProcess { process } => facade
            .scoped(session)
            .select_process(process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::ListProjects => Ok(IpcResponse::Projects(project_summaries(facade)?)),
        IpcRequest::GetProjectStatus { project } => project_status(facade, session, project),
        IpcRequest::ListProcesses => Ok(IpcResponse::Processes(
            facade.scoped(session).snapshot_scoped(),
        )),
        IpcRequest::GetProcessStatus { process } => facade
            .scoped(session)
            .process_status_scoped(process)
            .map(IpcResponse::Process)
            .map_err(IpcError::from),
        IpcRequest::StartProcess { process } => facade
            .scoped(session)
            .start_process(process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::StopProcess { process } => facade
            .scoped(session)
            .stop_process(process)
            .map(IpcResponse::Stopped)
            .map_err(IpcError::from),
        IpcRequest::RestartProcess { process } => facade
            .scoped(session)
            .restart_process(process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::RenameProcess { process, label } => facade
            .scoped(session)
            .rename_process(process, label)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::SpawnAgent { tool, extra_args } => facade
            .scoped(session)
            .spawn_agent(&tool, extra_args)
            .map(IpcResponse::Spawned)
            .map_err(IpcError::from),
        IpcRequest::ListAgentTools => facade
            .agents()
            .list_tools()
            .map(IpcResponse::AgentTools)
            .map_err(|err| IpcError::Internal(err.to_string())),
        IpcRequest::StartAllCommands => facade
            .scoped(session)
            .start_all_commands()
            .map(IpcResponse::BulkStarted)
            .map_err(IpcError::from),
        IpcRequest::StopAllCommands => facade
            .scoped(session)
            .stop_all_commands()
            .map(IpcResponse::BulkStopped)
            .map_err(IpcError::from),
        IpcRequest::RestartAllCommands => facade
            .scoped(session)
            .restart_all_commands()
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::GetProcessOutput { process, lines } => facade
            .scoped(session)
            .process_output_scoped(process, lines)
            .map(IpcResponse::Lines)
            .map_err(IpcError::from),
        IpcRequest::GetProcessRawOutput { process } => facade
            .scoped(session)
            .process_raw_output_scoped(process)
            .map(|bytes| IpcResponse::RawOutput(String::from_utf8_lossy(&bytes).into_owned()))
            .map_err(IpcError::from),
        IpcRequest::SearchOutput {
            process,
            query,
            limit,
        } => facade
            .scoped(session)
            .search_output_scoped(process, &query, limit)
            .map(IpcResponse::Lines)
            .map_err(IpcError::from),
        IpcRequest::SearchRawOutput {
            process,
            query,
            limit,
        } => facade
            .scoped(session)
            .search_raw_output_scoped(process, &query, limit)
            .map(IpcResponse::Lines)
            .map_err(IpcError::from),
        IpcRequest::ClearOutput { process } => facade
            .scoped(session)
            .clear_output(process)
            .map(|_| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::FlushTerminalPerf { process } => facade
            .scoped(session)
            .flush_terminal_perf_scoped(process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::GetProcessPorts { process } => facade
            .scoped(session)
            .process_ports_scoped(process)
            .map(IpcResponse::Ports)
            .map_err(IpcError::from),
        IpcRequest::ServicesList => facade
            .scoped(session)
            .services_list()
            .map(IpcResponse::Processes)
            .map_err(IpcError::from),
        IpcRequest::LockAcquire { key, ttl_ms } => facade
            .scoped(session)
            .lock_acquire(&key, ttl_ms.map(Duration::from_millis))
            .map(IpcResponse::LeaseOutcome)
            .map_err(IpcError::from),
        IpcRequest::LockStatus { key } => facade
            .scoped(session)
            .lock_status(&key)
            .map(IpcResponse::LeaseStatus)
            .map_err(IpcError::from),
        IpcRequest::LockRelease { key } => facade
            .scoped(session)
            .lock_release(&key)
            .map(IpcResponse::LeaseReleased)
            .map_err(IpcError::from),
        IpcRequest::TimerSet { body, after_ms } => facade
            .scoped(session)
            .timer_set(body, after_ms.map(Duration::from_millis))
            .map(IpcResponse::TimerArmed)
            .map_err(IpcError::from),
        IpcRequest::TimerFireWhenIdleAny {
            body,
            processes,
            max_wait_ms,
        } => facade
            .scoped(session)
            .timer_fire_when_idle(
                body,
                processes,
                IdleMode::Any,
                max_wait_ms.map(Duration::from_millis),
            )
            .map(IpcResponse::TimerWhenIdle)
            .map_err(IpcError::from),
        IpcRequest::TimerFireWhenIdleAll {
            body,
            processes,
            max_wait_ms,
        } => facade
            .scoped(session)
            .timer_fire_when_idle(
                body,
                processes,
                IdleMode::All,
                max_wait_ms.map(Duration::from_millis),
            )
            .map(IpcResponse::TimerWhenIdle)
            .map_err(IpcError::from),
        IpcRequest::TimerCancel { timer } => facade
            .scoped(session)
            .timer_cancel(timer)
            .map(IpcResponse::TimerChanged)
            .map_err(IpcError::from),
        IpcRequest::TimerPause { timer } => facade
            .scoped(session)
            .timer_pause(timer)
            .map(IpcResponse::TimerChanged)
            .map_err(IpcError::from),
        IpcRequest::TimerResume { timer } => facade
            .scoped(session)
            .timer_resume(timer)
            .map(IpcResponse::TimerChanged)
            .map_err(IpcError::from),
        IpcRequest::TimerList => facade
            .scoped(session)
            .timer_list()
            .map(IpcResponse::Timers)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadWrite {
            name,
            body,
            expected_revision,
        } => facade
            .scoped(session)
            .scratchpad_write(&name, body, expected_revision)
            .map(|written| IpcResponse::ScratchpadWritten {
                scratchpad: written.view,
                seeded_from: written.seeded_from,
            })
            .map_err(IpcError::from),
        IpcRequest::ScratchpadRead { name } => facade
            .scoped(session)
            .scratchpad_read(&name)
            .map(IpcResponse::Scratchpad)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadList => facade
            .scoped(session)
            .scratchpad_list()
            .map(IpcResponse::Scratchpads)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadRename { name, new_name } => facade
            .scoped(session)
            .scratchpad_rename(&name, &new_name)
            .map(IpcResponse::Scratchpad)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadAddTags { name, tags } => facade
            .scoped(session)
            .scratchpad_add_tags(&name, &tags)
            .map(IpcResponse::Scratchpad)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadRemoveTags { name, tags } => facade
            .scoped(session)
            .scratchpad_remove_tags(&name, &tags)
            .map(IpcResponse::Scratchpad)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadTagsList => facade
            .scoped(session)
            .scratchpad_tags_list()
            .map(IpcResponse::ScratchpadTags)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadArchive { name, archived } => facade
            .scoped(session)
            .scratchpad_archive(&name, archived)
            .map(IpcResponse::Scratchpad)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadDelete { name } => facade
            .scoped(session)
            .scratchpad_delete(&name)
            .map(IpcResponse::ScratchpadDeleted)
            .map_err(IpcError::from),
        IpcRequest::ScratchpadTransfer { name, to_project } => facade
            .scoped(session)
            .scratchpad_transfer(&name, to_project)
            .map(IpcResponse::Scratchpad)
            .map_err(IpcError::from),
        IpcRequest::TodoCreate { doc, scratchpad } => facade
            .scoped(session)
            .todo_create(doc, scratchpad)
            .map(|created| IpcResponse::TodoCreated {
                todo: created.view,
                seeded_from: created.seeded_from,
            })
            .map_err(IpcError::from),
        IpcRequest::TodoList => facade
            .scoped(session)
            .todo_list()
            .map(IpcResponse::Todos)
            .map_err(IpcError::from),
        IpcRequest::TodoGet { todo } => facade
            .scoped(session)
            .todo_get(todo)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoUpdate {
            todo,
            doc,
            scratchpad,
            expected_revision,
        } => facade
            .scoped(session)
            .todo_update(todo, doc, scratchpad, expected_revision)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoComplete { todo } => facade
            .scoped(session)
            .todo_complete(todo)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoDelete { todo } => facade
            .scoped(session)
            .todo_delete(todo)
            .map(IpcResponse::TodoDeleted)
            .map_err(IpcError::from),
        IpcRequest::TodoTransfer { todo, to_project } => facade
            .scoped(session)
            .todo_transfer(to_project, todo)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoTagsList => facade
            .scoped(session)
            .todo_tags_list()
            .map(IpcResponse::TodoTags)
            .map_err(IpcError::from),
        IpcRequest::TodoAddTag { todo, tag } => facade
            .scoped(session)
            .todo_add_tag(todo, &tag)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoRemoveTag { todo, tag } => facade
            .scoped(session)
            .todo_remove_tag(todo, &tag)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoSetBlockers { todo, blockers } => facade
            .scoped(session)
            .todo_set_blockers(todo, blockers)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoAddBlocker { todo, blocker } => facade
            .scoped(session)
            .todo_add_blocker(todo, blocker)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoRemoveBlocker { todo, blocker } => facade
            .scoped(session)
            .todo_remove_blocker(todo, blocker)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoLock { todo } => facade
            .scoped(session)
            .todo_lock(todo)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoUnlock { todo } => facade
            .scoped(session)
            .todo_unlock(todo)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoCommentCreate { todo, body } => facade
            .scoped(session)
            .todo_comment_create(todo, &body)
            .map(|(todo, comment)| IpcResponse::TodoComment { todo, comment })
            .map_err(IpcError::from),
        IpcRequest::TodoCommentUpdate {
            todo,
            comment,
            body,
        } => facade
            .scoped(session)
            .todo_comment_update(todo, comment, &body)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoCommentDelete { todo, comment } => facade
            .scoped(session)
            .todo_comment_delete(todo, comment)
            .map(IpcResponse::Todo)
            .map_err(IpcError::from),
        IpcRequest::TodoCommentList { todo } => facade
            .scoped(session)
            .todo_comment_list(todo)
            .map(IpcResponse::TodoComments)
            .map_err(IpcError::from),
        IpcRequest::ResolveLink { link } => facade
            .scoped(session)
            .resolve_link(&link)
            .map(IpcResponse::Link)
            .map_err(IpcError::from),
        IpcRequest::KvSet { key, value } => facade
            .scoped(session)
            .kv_set(key, value)
            .map(|()| IpcResponse::KvValue(None))
            .map_err(IpcError::from),
        IpcRequest::KvGet { key } => facade
            .scoped(session)
            .kv_get(key)
            .map(IpcResponse::KvValue)
            .map_err(IpcError::from),
        IpcRequest::KvDelete { key } => facade
            .scoped(session)
            .kv_delete(key)
            .map(IpcResponse::KvDeleted)
            .map_err(IpcError::from),
        IpcRequest::KvList => facade
            .scoped(session)
            .kv_list()
            .map(IpcResponse::KvPairs)
            .map_err(IpcError::from),
        IpcRequest::McpToolGroups => facade
            .mcp_tool_groups()
            .map(IpcResponse::McpToolGroups)
            .map_err(|err| IpcError::Internal(err.to_string())),
        IpcRequest::SubmitFeedback { message } => facade
            .submit_feedback(&message)
            .map(IpcResponse::Feedback)
            .map_err(IpcError::from),
        IpcRequest::SetupAgentIntegration { file } => facade
            .scoped(session)
            .setup_agent_integration(file)
            .map(IpcResponse::IntegrationWritten)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateList { scope } => facade
            .scoped(session)
            .prompt_template_list(scope)
            .map(IpcResponse::PromptTemplates)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateRead { scope, name } => facade
            .scoped(session)
            .prompt_template_read(scope, &name)
            .map(IpcResponse::PromptTemplate)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateCreate {
            scope,
            name,
            description,
            body,
        } => facade
            .scoped(session)
            .prompt_template_create(scope, &name, description.as_deref(), &body)
            .map(IpcResponse::PromptTemplate)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateUpdate {
            scope,
            name,
            description,
            body,
            expected_revision,
        } => facade
            .scoped(session)
            .prompt_template_update(
                scope,
                &name,
                description.as_deref(),
                &body,
                expected_revision,
            )
            .map(IpcResponse::PromptTemplate)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateDelete { scope, name } => facade
            .scoped(session)
            .prompt_template_delete(scope, &name)
            .map(IpcResponse::PromptTemplateDeleted)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateExport { scope, name } => facade
            .scoped(session)
            .prompt_template_export(scope, &name)
            .map(IpcResponse::PromptTemplateExport)
            .map_err(IpcError::from),
        IpcRequest::PromptTemplateRender {
            scope,
            name,
            values,
            policy,
        } => facade
            .scoped(session)
            .prompt_template_render(
                scope,
                &RenderRequest {
                    name,
                    values,
                    policy,
                },
            )
            .map(IpcResponse::PromptTemplateRendered)
            .map_err(IpcError::from),
    }
}

/// Every loaded project as a lean, agent-facing summary.
fn project_summaries(facade: &Facade) -> Result<Vec<ProjectSummary>, IpcError> {
    Ok(facade
        .projects_snapshot()
        .map_err(|err| IpcError::Internal(err.to_string()))?
        .iter()
        .map(ProjectSummary::from_view)
        .collect())
}

/// One project (explicit, or the session's effective scope) with its current processes.
fn project_status(facade: &Facade, session: SessionId, project: Option<ProjectId>) -> IpcResult {
    let target = match project {
        Some(project) => project,
        None => facade
            .effective_project(session)
            .ok_or(IpcError::NoProjectScope)?,
    };
    let view = facade
        .projects_snapshot()
        .map_err(|err| IpcError::Internal(err.to_string()))?
        .into_iter()
        .find(|view| view.id == target)
        .ok_or(IpcError::UnknownProject)?;
    let processes = facade.scoped(session).project_processes_scoped(target);
    Ok(IpcResponse::ProjectStatus(ProjectStatus {
        project: ProjectSummary::from_view(&view),
        processes,
    }))
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
