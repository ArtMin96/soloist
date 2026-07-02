use super::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use soloist_core::{
    AcquireOutcome, AgentKind, AgentTool, Comment, FireCond, LeaseView, LinkContent, McpToolGroups,
    Origin, ProcStatus, ProcessId, ProcessKind, ProcessView, ProjectId, PromptMode, Readiness,
    ScratchpadDoc, ScratchpadId, ScratchpadSummary, ScratchpadView, SessionId, SetWhenIdleOutcome,
    StartSummary, TimerId, TimerStatus, TimerView, TodoDoc, TodoId, TodoStatus, TodoSummary,
    TodoView, Whoami,
};
use soloist_core::{
    FeedbackEntry, IntegrationFile, IntegrationWrite, PromptScope, PromptTemplateId,
    PromptTemplateView,
};
use soloist_ipc::{
    read_frame, write_frame, IpcError, IpcRequest, IpcResponse, IpcResult, PortWaitOutcome,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use tokio::net::UnixListener;

use crate::args::{
    IntegrationFileArg, LockAcquireArg, LockKeyArg, OutputArg, ProcessArg, PromptScopeArg,
    PromptTemplateCreateArg, PromptTemplateUpdateArg, RenameArg, ScratchpadArchiveArg,
    ScratchpadNameArg, ScratchpadTagsArg, ScratchpadWriteArg, SearchArg, SelectProjectArg,
    SendInputArg, SetupAgentIntegrationArg, SpawnAgentArg, SubmitFeedbackArg, TimerArg,
    TimerFireWhenIdleArg, TimerSetArg, TodoArg, TodoCommentCreateArg, TodoCreateArg, TodoGetArg,
    TodoRef, TodoStatusArg, WaitForPortArg,
};

/// Spawns a fake app on `socket` that answers each request via `respond` until the client
/// disconnects, so a test drives the real [`SoloistMcp`] handler through the real IPC
/// transport — exercising tool dispatch, response projection, and error mapping end to end.
fn spawn_fake_app(socket: PathBuf, respond: impl Fn(IpcRequest) -> IpcResult + Send + 'static) {
    let listener = UnixListener::bind(&socket).expect("bind");
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        while let Some(request) = read_frame::<_, IpcRequest>(&mut stream)
            .await
            .expect("read request")
        {
            let reply = respond(request);
            write_frame(&mut stream, &reply).await.expect("write reply");
        }
    });
}

/// Every feature group enabled — the full tool surface, so the per-tool tests below exercise every
/// tool regardless of the default gating (the gating tests construct specific enablements).
fn all_feature_groups() -> McpToolGroups {
    McpToolGroups {
        scratchpads: true,
        todos: true,
        timers: true,
        key_value: true,
        prompt_templates: true,
    }
}

/// A handler whose single client connection talks to the fake app on `socket`, with every feature
/// group enabled.
fn handler(socket: PathBuf) -> SoloistMcp {
    SoloistMcp::new(Arc::new(AppClient::new(None, socket)), all_feature_groups())
}

/// A handler with the given feature-group enablement. `list_all` reads the statically composed
/// router, so no IPC connection is opened and the socket path is never used.
fn handler_with_groups(groups: McpToolGroups) -> SoloistMcp {
    SoloistMcp::new(
        Arc::new(AppClient::new(None, PathBuf::from("unused.sock"))),
        groups,
    )
}

/// The tool names the composed router actually serves.
fn served_tools(handler: &SoloistMcp) -> BTreeSet<String> {
    handler
        .tool_router
        .list_all()
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect()
}

/// The structured JSON content a tool returned, or a panic if there was none.
fn structured_of(result: CallToolResult) -> serde_json::Value {
    result.structured_content.expect("a structured tool result")
}

fn sample_view(id: u64) -> ProcessView {
    ProcessView {
        id: ProcessId::from_raw(id),
        project: ProjectId::from_raw(1),
        kind: ProcessKind::Terminal,
        label: "term".into(),
        status: ProcStatus::Running,
        exit_code: None,
        requires_trust: false,
        resumable: false,
        ports: Vec::new(),
        ready: Readiness::Ungated,
    }
}

/// The tool surface the MCP server is meant to expose, as an explicit list — one entry per
/// `#[tool]`, grouped by the category file that defines it. This is the single source of the
/// *intended* surface, kept deliberately separate from the router that produces the *actual*
/// surface, so the assertion below compares two independent things rather than restating one.
const EXPECTED_TOOL_SURFACE: &[&str] = &[
    // tools/identity.rs
    "whoami",
    "register_agent",
    "select_project",
    "select_process",
    // tools/project.rs
    "list_projects",
    "get_project_status",
    // tools/process.rs
    "list_processes",
    "get_process_status",
    "start_process",
    "stop_process",
    "restart_process",
    "rename_process",
    "close_process",
    "send_input",
    // tools/agent.rs
    "spawn_agent",
    "list_agent_tools",
    // tools/bulk.rs
    "start_all_commands",
    "stop_all_commands",
    "restart_all_commands",
    // tools/output.rs
    "get_process_output",
    "get_process_raw_output",
    "search_output",
    "search_raw_output",
    "clear_output",
    "flush_terminal_perf",
    "get_process_ports",
    // tools/services.rs
    "services_list",
    "wait_for_bound_port",
    // tools/lock.rs
    "lock_acquire",
    "lock_status",
    "lock_release",
    // tools/setup.rs
    "help",
    "submit_solo_feedback",
    "setup_agent_integration",
    // tools/timer.rs
    "timer_set",
    "timer_fire_when_idle_any",
    "timer_fire_when_idle_all",
    "timer_cancel",
    "timer_pause",
    "timer_resume",
    "timer_list",
    // tools/scratchpad.rs
    "scratchpad_list",
    "scratchpad_read",
    "scratchpad_write",
    "scratchpad_rename",
    "scratchpad_add_tags",
    "scratchpad_remove_tags",
    "scratchpad_tags_list",
    "scratchpad_archive",
    "scratchpad_delete",
    "scratchpad_transfer",
    // tools/todo.rs
    "todo_list",
    "todo_get",
    "todo_create",
    "todo_update",
    "todo_complete",
    "todo_delete",
    "todo_transfer",
    "todo_tags_list",
    "todo_add_tag",
    "todo_remove_tag",
    "todo_set_blockers",
    "todo_add_blocker",
    "todo_remove_blocker",
    "todo_lock",
    "todo_unlock",
    "todo_comment_create",
    "todo_comment_update",
    "todo_comment_delete",
    "todo_comment_list",
    // tools/kv.rs
    "kv_set",
    "kv_get",
    "kv_delete",
    "kv_list",
    // tools/prompt_template.rs
    "prompt_template_list",
    "prompt_template_read",
    "prompt_template_create",
    "prompt_template_update",
    "prompt_template_delete",
    "prompt_template_export",
];

/// With every feature group enabled, the router [`SoloistMcp::new`] composes from the per-category
/// sub-routers must serve exactly the intended surface. This guards the split itself: a category
/// sub-router left out of the `+` composition, a tool name colliding across two category files
/// (`ToolRouter::add_route` is a map insert that silently overwrites), or an accidental add/rename
/// all change what the served router reports — drift the per-tool behaviour tests cannot catch,
/// because they invoke the tool methods directly without going through the composed router.
#[test]
fn served_router_exposes_exactly_the_expected_tool_surface() {
    let dir = tempfile::tempdir().expect("temp dir");
    // `list_all` reads the statically composed router; no IPC connection is opened, so the
    // socket path is never touched. `handler` enables every feature group.
    let served = served_tools(&handler(dir.path().join("soloist-ipc.sock")));

    let expected: BTreeSet<String> = EXPECTED_TOOL_SURFACE
        .iter()
        .map(|name| name.to_string())
        .collect();

    assert_eq!(
        served, expected,
        "the served MCP tool surface drifted from the intended set"
    );
}

/// The default surface serves every core and on-by-default feature group, but gates Key-Value off
/// (G10): its tools are absent from the composed router, so they are neither listed nor callable.
#[test]
fn default_settings_gate_key_value_off_and_serve_the_rest() {
    let served = served_tools(&handler_with_groups(McpToolGroups::default()));

    // Core groups and the on-by-default feature groups are present.
    assert!(served.contains("whoami"), "a core tool is served");
    assert!(
        served.contains("lock_acquire"),
        "coordination leases are a core group"
    );
    assert!(
        served.contains("help"),
        "setup/support is a core group, always served"
    );
    assert!(served.contains("scratchpad_list"));
    assert!(served.contains("todo_list"));
    assert!(served.contains("timer_set"));
    // Key-Value and Prompt Templates are gated off by default.
    assert!(
        !served.iter().any(|name| name.starts_with("kv_")),
        "no Key-Value tool is served by default"
    );
    assert!(
        !served
            .iter()
            .any(|name| name.starts_with("prompt_template_")),
        "no Prompt Template tool is served by default"
    );
}

/// Enabling Key-Value adds its tools to the served surface.
#[test]
fn enabling_key_value_serves_its_tools() {
    let groups = McpToolGroups {
        key_value: true,
        ..McpToolGroups::default()
    };
    let served = served_tools(&handler_with_groups(groups));

    for tool in ["kv_set", "kv_get", "kv_delete", "kv_list"] {
        assert!(
            served.contains(tool),
            "{tool} should be served when Key-Value is enabled"
        );
    }
}

/// Enabling Prompt Templates adds its tools to the served surface.
#[test]
fn enabling_prompt_templates_serves_its_tools() {
    let groups = McpToolGroups {
        prompt_templates: true,
        ..McpToolGroups::default()
    };
    let served = served_tools(&handler_with_groups(groups));

    for tool in [
        "prompt_template_list",
        "prompt_template_read",
        "prompt_template_create",
        "prompt_template_update",
        "prompt_template_delete",
        "prompt_template_export",
    ] {
        assert!(
            served.contains(tool),
            "{tool} should be served when Prompt Templates is enabled"
        );
    }
}

/// Disabling one feature group hides only its tools — the core groups and the other feature groups
/// are unaffected.
#[test]
fn disabling_a_feature_group_hides_only_its_tools() {
    let groups = McpToolGroups {
        scratchpads: false,
        todos: true,
        timers: true,
        key_value: false,
        prompt_templates: false,
    };
    let served = served_tools(&handler_with_groups(groups));

    assert!(
        !served.iter().any(|name| name.starts_with("scratchpad_")),
        "the disabled group's tools are gone"
    );
    assert!(
        served.contains("todo_list"),
        "another feature group is unaffected"
    );
    assert!(
        served.contains("timer_set"),
        "another feature group is unaffected"
    );
    assert!(served.contains("whoami"), "core tools stay regardless");
}

#[tokio::test]
async fn whoami_projects_the_resolved_identity() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let who = Whoami {
        session: SessionId::from_raw(1),
        origin: Origin::Unbound,
        bound_process: None,
        selected_process: None,
        effective_project: None,
    };
    let canned = who.clone();
    spawn_fake_app(socket.clone(), move |_request| {
        Ok(IpcResponse::Whoami(canned.clone()))
    });

    let result = handler(socket).whoami().await.expect("whoami succeeds");
    let back: Whoami = serde_json::from_value(structured_of(result)).expect("decode whoami");
    assert_eq!(back, who);
}

#[tokio::test]
async fn list_processes_projects_the_process_rows() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let view = sample_view(7);
    let canned = view.clone();
    spawn_fake_app(socket.clone(), move |_request| {
        Ok(IpcResponse::Processes(vec![canned.clone()]))
    });

    let result = handler(socket)
        .list_processes()
        .await
        .expect("list succeeds");
    let back: Vec<ProcessView> =
        serde_json::from_value(structured_of(result)).expect("decode processes");
    assert_eq!(back, vec![view]);
}

#[tokio::test]
async fn get_process_status_threads_the_id_through_to_the_app() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // Echo the requested id back as the view's id, proving the argument reaches the app.
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::GetProcessStatus { process } => {
            Ok(IpcResponse::Process(sample_view(process.get())))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .get_process_status(Parameters(ProcessArg { process: 7 }))
        .await
        .expect("status succeeds");
    let back: ProcessView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.id, ProcessId::from_raw(7));
}

#[tokio::test]
async fn select_project_acknowledges() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SelectProject { project } if project == ProjectId::from_raw(3) => {
            Ok(IpcResponse::Acked)
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .select_project(Parameters(SelectProjectArg { project: 3 }))
        .await
        .expect("select succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn a_request_error_becomes_a_tool_execution_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |_request| Err(IpcError::UnknownProcess));

    // A request-caused error is a tool-execution error (isError: true) the model can read
    // and self-correct on — not a protocol error it cannot recover from.
    let result = handler(socket)
        .get_process_status(Parameters(ProcessArg { process: 99 }))
        .await
        .expect("a request error is a tool result, not a protocol error");
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn a_server_error_stays_a_protocol_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // A server-side failure is not something the model can fix by adjusting parameters, so it
    // surfaces as a protocol error rather than a tool-execution error.
    spawn_fake_app(socket.clone(), |_request| {
        Err(IpcError::Internal("disk full".into()))
    });

    let result = handler(socket)
        .start_process(Parameters(ProcessArg { process: 5 }))
        .await;
    assert!(result.is_err(), "a server error is a protocol error");
}

#[tokio::test]
async fn a_wrong_shaped_reply_is_a_protocol_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The app answers a whoami with the wrong variant — a protocol mismatch.
    spawn_fake_app(socket.clone(), |_request| Ok(IpcResponse::Acked));

    let result = handler(socket).whoami().await;
    assert!(result.is_err(), "a mismatched reply must be rejected");
}

#[tokio::test]
async fn start_process_threads_the_id_through_and_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::StartProcess { process } if process == ProcessId::from_raw(5) => {
            Ok(IpcResponse::Acked)
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .start_process(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("start succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn stop_process_reports_whether_it_was_running() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::StopProcess { .. } => Ok(IpcResponse::Stopped(true)),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .stop_process(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("stop succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "was_running": true })
    );
}

#[tokio::test]
async fn restart_process_threads_the_id_through_and_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::RestartProcess { process } if process == ProcessId::from_raw(5) => {
            Ok(IpcResponse::Acked)
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .restart_process(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("restart succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn rename_process_threads_its_arguments_through_and_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::RenameProcess { process, label }
            if process == ProcessId::from_raw(5) && label == "worker" =>
        {
            Ok(IpcResponse::Acked)
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .rename_process(Parameters(RenameArg {
            process: 5,
            label: "worker".into(),
        }))
        .await
        .expect("rename succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn close_process_threads_the_id_through_and_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::CloseProcess { process } if process == ProcessId::from_raw(5) => {
            Ok(IpcResponse::Acked)
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .close_process(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("close succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn select_process_threads_the_id_through_and_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SelectProcess { process } if process == ProcessId::from_raw(5) => {
            Ok(IpcResponse::Acked)
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .select_process(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("select succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn send_input_threads_its_arguments_through_and_returns_the_tail() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SendInput {
            process,
            input,
            wait_ms,
        } if process == ProcessId::from_raw(5) && input == "ls\r" && wait_ms == Some(200) => {
            Ok(IpcResponse::InputSent(Some("$ ls\nfile.txt".into())))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .send_input(Parameters(SendInputArg {
            process: 5,
            input: "ls\r".into(),
            wait_ms: Some(200),
        }))
        .await
        .expect("send_input succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "tail": "$ ls\nfile.txt" })
    );
}

#[tokio::test]
async fn send_input_without_a_wait_returns_a_null_tail() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SendInput { .. } => Ok(IpcResponse::InputSent(None)),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .send_input(Parameters(SendInputArg {
            process: 5,
            input: "x".into(),
            wait_ms: None,
        }))
        .await
        .expect("send_input succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "tail": null }));
}

#[tokio::test]
async fn spawn_agent_threads_its_arguments_through_and_returns_the_process_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SpawnAgent { tool, extra_args }
            if tool == "Claude" && extra_args == ["--model", "opus"] =>
        {
            Ok(IpcResponse::Spawned(ProcessId::from_raw(42)))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .spawn_agent(Parameters(SpawnAgentArg {
            tool: "Claude".into(),
            extra_args: vec!["--model".into(), "opus".into()],
        }))
        .await
        .expect("spawn_agent succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "process": 42 }));
}

#[tokio::test]
async fn a_workers_spawn_refusal_is_a_tool_error_the_model_can_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SpawnAgent { .. } => Err(IpcError::WorkerMayNotSpawn),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    // The one-level delegation refusal is caller-fixable feedback: a tool-execution error
    // carrying the message, not a protocol error.
    let result = handler(socket)
        .spawn_agent(Parameters(SpawnAgentArg {
            tool: "worker".into(),
            extra_args: Vec::new(),
        }))
        .await
        .expect("a request error is a tool result, not a protocol error");
    assert_eq!(result.is_error, Some(true));
    let rendered = serde_json::to_value(&result)
        .expect("serialize the tool result")
        .to_string();
    assert!(
        rendered.contains("a worker agent cannot spawn agents"),
        "the refusal message reaches the model: {rendered}",
    );
}

#[tokio::test]
async fn list_agent_tools_projects_the_configured_tools() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let tool = AgentTool {
        name: "Claude".into(),
        command: "claude".into(),
        default_args: Vec::new(),
        kind: AgentKind::Claude,
        prompt_mode: PromptMode::AppendedArg,
    };
    let canned = tool.clone();
    spawn_fake_app(socket.clone(), move |_request| {
        Ok(IpcResponse::AgentTools(vec![canned.clone()]))
    });

    let result = handler(socket)
        .list_agent_tools()
        .await
        .expect("list_agent_tools succeeds");
    let back: Vec<AgentTool> = serde_json::from_value(structured_of(result)).expect("decode tools");
    assert_eq!(back, vec![tool]);
}

#[tokio::test]
async fn start_all_commands_projects_the_summary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let summary = StartSummary {
        started: vec![ProcessId::from_raw(1), ProcessId::from_raw(2)],
        skipped_untrusted: vec![ProcessId::from_raw(3)],
    };
    let canned = summary.clone();
    spawn_fake_app(socket.clone(), move |request| match request {
        IpcRequest::StartAllCommands => Ok(IpcResponse::BulkStarted(canned.clone())),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .start_all_commands()
        .await
        .expect("start_all_commands succeeds");
    let back: StartSummary = serde_json::from_value(structured_of(result)).expect("decode summary");
    assert_eq!(back, summary);
}

#[tokio::test]
async fn stop_all_commands_reports_the_count() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::StopAllCommands => Ok(IpcResponse::BulkStopped(2)),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .stop_all_commands()
        .await
        .expect("stop_all_commands succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "stopped": 2 }));
}

#[tokio::test]
async fn restart_all_commands_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::RestartAllCommands => Ok(IpcResponse::Acked),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .restart_all_commands()
        .await
        .expect("restart_all_commands succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn get_process_output_projects_the_lines() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::GetProcessOutput {
            process,
            lines: Some(50),
        } if process == ProcessId::from_raw(5) => {
            Ok(IpcResponse::Lines(vec!["line a".into(), "line b".into()]))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .get_process_output(Parameters(OutputArg {
            process: 5,
            lines: Some(50),
        }))
        .await
        .expect("get_process_output succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "output": ["line a", "line b"] })
    );
}

#[tokio::test]
async fn search_output_threads_the_query_and_projects_matches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SearchOutput { query, .. } if query == "error" => {
            Ok(IpcResponse::Lines(vec!["error: boom".into()]))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .search_output(Parameters(SearchArg {
            process: 5,
            query: "error".into(),
            limit: None,
        }))
        .await
        .expect("search_output succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "matches": ["error: boom"] })
    );
}

#[tokio::test]
async fn get_process_ports_projects_the_ports() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::GetProcessPorts { .. } => Ok(IpcResponse::Ports(vec![3000, 8080])),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .get_process_ports(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("get_process_ports succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "ports": [3000, 8080] })
    );
}

#[tokio::test]
async fn clear_output_acks() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ClearOutput { .. } => Ok(IpcResponse::Acked),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .clear_output(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("clear_output succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn services_list_projects_the_services() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let view = sample_view(7);
    let canned = view.clone();
    spawn_fake_app(socket.clone(), move |request| match request {
        IpcRequest::ServicesList => Ok(IpcResponse::Processes(vec![canned.clone()])),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .services_list()
        .await
        .expect("services_list succeeds");
    let services = structured_of(result);
    let back: Vec<ProcessView> =
        serde_json::from_value(services["services"].clone()).expect("decode services");
    assert_eq!(back, vec![view]);
}

#[tokio::test]
async fn wait_for_bound_port_projects_a_bound_outcome() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::WaitForBoundPort { port: 3000, .. } => {
            Ok(IpcResponse::PortWait(PortWaitOutcome::Bound))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .wait_for_bound_port(Parameters(WaitForPortArg {
            process: 5,
            port: 3000,
            timeout_ms: None,
        }))
        .await
        .expect("wait_for_bound_port succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "bound": true }));
}

#[tokio::test]
async fn wait_for_bound_port_projects_a_timeout_reason() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |_request| {
        Ok(IpcResponse::PortWait(PortWaitOutcome::TimedOut))
    });

    let result = handler(socket)
        .wait_for_bound_port(Parameters(WaitForPortArg {
            process: 5,
            port: 3000,
            timeout_ms: Some(100),
        }))
        .await
        .expect("wait_for_bound_port succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "bound": false, "reason": "timed_out" })
    );
}

#[tokio::test]
async fn a_refused_action_becomes_a_tool_execution_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The core refused the action (untrusted / out of scope); the agent must see it as an
    // actionable tool error carrying the reason, so it can ask the user to trust the command.
    spawn_fake_app(socket.clone(), |_request| Err(IpcError::Untrusted));

    let result = handler(socket)
        .start_process(Parameters(ProcessArg { process: 5 }))
        .await
        .expect("a refusal is a tool result, not a protocol error");
    assert_eq!(result.is_error, Some(true));
    let json = serde_json::to_string(&result).expect("serialize result");
    assert!(
        json.contains("not trusted"),
        "the refusal reason reaches the model: {json}"
    );
}

#[tokio::test]
async fn lock_acquire_threads_its_arguments_through_and_projects_the_outcome() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::LockAcquire { key, ttl_ms } if key == "deploy" && ttl_ms == Some(30_000) => Ok(
            IpcResponse::LeaseOutcome(AcquireOutcome::Acquired(LeaseView {
                key: "deploy".into(),
                owner: ProcessId::from_raw(7),
                expires_unix_millis: 1_700_000_030_000,
            })),
        ),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .lock_acquire(Parameters(LockAcquireArg {
            key: "deploy".into(),
            ttl_ms: Some(30_000),
        }))
        .await
        .expect("lock_acquire succeeds");
    let back: AcquireOutcome =
        serde_json::from_value(structured_of(result)).expect("decode acquire outcome");
    assert!(matches!(back, AcquireOutcome::Acquired(view) if view.owner == ProcessId::from_raw(7)));
}

#[tokio::test]
async fn lock_acquire_forwards_an_omitted_ttl_as_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The default and the bounds live in the core, so the handler forwards an omitted ttl as None
    // rather than inventing one — this succeeds only if it sent exactly that.
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::LockAcquire { key, ttl_ms: None } if key == "deploy" => Ok(
            IpcResponse::LeaseOutcome(AcquireOutcome::Acquired(LeaseView {
                key,
                owner: ProcessId::from_raw(1),
                expires_unix_millis: 0,
            })),
        ),
        _ => Err(IpcError::Internal("expected an absent ttl".into())),
    });

    handler(socket)
        .lock_acquire(Parameters(LockAcquireArg {
            key: "deploy".into(),
            ttl_ms: None,
        }))
        .await
        .expect("lock_acquire forwards the absent ttl");
}

#[tokio::test]
async fn lock_status_projects_the_holder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::LockStatus { key } if key == "deploy" => {
            Ok(IpcResponse::LeaseStatus(Some(LeaseView {
                key: "deploy".into(),
                owner: ProcessId::from_raw(7),
                expires_unix_millis: 1_700_000_030_000,
            })))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .lock_status(Parameters(LockKeyArg {
            key: "deploy".into(),
        }))
        .await
        .expect("lock_status succeeds");
    let holder: LeaseView =
        serde_json::from_value(structured_of(result)["holder"].clone()).expect("decode holder");
    assert_eq!(holder.owner, ProcessId::from_raw(7));
}

#[tokio::test]
async fn lock_status_reports_a_free_key_as_a_null_holder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |_request| {
        Ok(IpcResponse::LeaseStatus(None))
    });

    let result = handler(socket)
        .lock_status(Parameters(LockKeyArg { key: "free".into() }))
        .await
        .expect("lock_status succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "holder": null }));
}

#[tokio::test]
async fn lock_release_reports_whether_the_caller_held_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::LockRelease { key } if key == "deploy" => Ok(IpcResponse::LeaseReleased(true)),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .lock_release(Parameters(LockKeyArg {
            key: "deploy".into(),
        }))
        .await
        .expect("lock_release succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "released": true })
    );
}

/// A sample armed timer for the response-projection tests.
fn sample_timer(id: u64) -> TimerView {
    TimerView {
        id: TimerId::from_raw(id),
        owner: ProcessId::from_raw(1),
        body: "resume work".into(),
        fire: FireCond::At,
        status: TimerStatus::Armed,
        deadline_unix_millis: 1_700_000_005_000,
        waiting_on: vec![],
        already_idle: false,
        paused_remaining_millis: None,
    }
}

#[tokio::test]
async fn timer_set_threads_its_arguments_through_and_projects_the_timer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TimerSet { body, after_ms }
            if body == "resume work" && after_ms == Some(5_000) =>
        {
            Ok(IpcResponse::TimerArmed(sample_timer(3)))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .timer_set(Parameters(TimerSetArg {
            body: "resume work".into(),
            after_ms: Some(5_000),
        }))
        .await
        .expect("timer_set succeeds");
    let back: TimerView = serde_json::from_value(structured_of(result)).expect("decode timer");
    assert_eq!(back.id, TimerId::from_raw(3));
}

#[tokio::test]
async fn timer_fire_when_idle_all_threads_the_processes_and_projects_the_outcome() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TimerFireWhenIdleAll {
            body,
            processes,
            max_wait_ms,
        } if body == "all done"
            && processes == vec![ProcessId::from_raw(2), ProcessId::from_raw(3)]
            && max_wait_ms == Some(60_000) =>
        {
            Ok(IpcResponse::TimerWhenIdle(SetWhenIdleOutcome {
                timer: TimerView {
                    id: TimerId::from_raw(9),
                    owner: ProcessId::from_raw(1),
                    body,
                    fire: FireCond::WhenIdleAll {
                        watched: processes.clone(),
                    },
                    status: TimerStatus::Armed,
                    deadline_unix_millis: 0,
                    waiting_on: processes.clone(),
                    already_idle: false,
                    paused_remaining_millis: None,
                },
                already_idle: false,
                waiting_on: processes,
            }))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .timer_fire_when_idle_all(Parameters(TimerFireWhenIdleArg {
            body: "all done".into(),
            processes: vec![2, 3],
            max_wait_ms: Some(60_000),
        }))
        .await
        .expect("timer_fire_when_idle_all succeeds");
    let back: SetWhenIdleOutcome =
        serde_json::from_value(structured_of(result)).expect("decode outcome");
    assert!(!back.already_idle);
    assert_eq!(
        back.waiting_on,
        vec![ProcessId::from_raw(2), ProcessId::from_raw(3)]
    );
}

#[tokio::test]
async fn timer_fire_when_idle_any_uses_the_any_request_variant() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TimerFireWhenIdleAny { processes, .. }
            if processes == vec![ProcessId::from_raw(4)] =>
        {
            Ok(IpcResponse::TimerWhenIdle(SetWhenIdleOutcome {
                timer: sample_timer(1),
                already_idle: true,
                waiting_on: Vec::new(),
            }))
        }
        _ => Err(IpcError::Internal("expected the any variant".into())),
    });

    let result = handler(socket)
        .timer_fire_when_idle_any(Parameters(TimerFireWhenIdleArg {
            body: "one done".into(),
            processes: vec![4],
            max_wait_ms: None,
        }))
        .await
        .expect("timer_fire_when_idle_any succeeds");
    let back: SetWhenIdleOutcome =
        serde_json::from_value(structured_of(result)).expect("decode outcome");
    assert!(back.already_idle, "the any-outcome is projected");
}

#[tokio::test]
async fn timer_cancel_reports_whether_it_was_cancelled() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TimerCancel { timer } if timer == TimerId::from_raw(5) => {
            Ok(IpcResponse::TimerChanged(true))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .timer_cancel(Parameters(TimerArg { timer: 5 }))
        .await
        .expect("timer_cancel succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "cancelled": true })
    );
}

#[tokio::test]
async fn timer_pause_reports_whether_it_was_paused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TimerPause { timer } if timer == TimerId::from_raw(5) => {
            Ok(IpcResponse::TimerChanged(true))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .timer_pause(Parameters(TimerArg { timer: 5 }))
        .await
        .expect("timer_pause succeeds");
    assert_eq!(structured_of(result), serde_json::json!({ "paused": true }));
}

#[tokio::test]
async fn timer_resume_reports_whether_it_was_resumed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TimerResume { timer } if timer == TimerId::from_raw(5) => {
            Ok(IpcResponse::TimerChanged(false))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .timer_resume(Parameters(TimerArg { timer: 5 }))
        .await
        .expect("timer_resume succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "resumed": false })
    );
}

#[tokio::test]
async fn timer_list_projects_the_timers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let timer = sample_timer(3);
    let canned = timer.clone();
    spawn_fake_app(socket.clone(), move |request| match request {
        IpcRequest::TimerList => Ok(IpcResponse::Timers(vec![canned.clone()])),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .timer_list()
        .await
        .expect("timer_list succeeds");
    let back: Vec<TimerView> =
        serde_json::from_value(structured_of(result)["timers"].clone()).expect("decode timers");
    assert_eq!(back, vec![timer]);
}

/// A sample disciplined document for the scratchpad response-projection tests.
fn sample_doc() -> ScratchpadDoc {
    ScratchpadDoc {
        objective: "Ship v1".into(),
        context: "RC cut".into(),
        plan: vec!["Cut RC".into()],
        acceptance_criteria: vec!["soak green".into()],
        risks: vec!["none identified".into()],
        status: "in progress".into(),
        notes: None,
    }
}

/// A sample scratchpad view for the response-projection tests.
fn sample_scratchpad(name: &str) -> ScratchpadView {
    let doc = sample_doc();
    let rendered = doc.render(name);
    ScratchpadView {
        id: ScratchpadId::from_raw(1),
        name: name.into(),
        tags: vec!["release".into()],
        archived: false,
        revision: 1,
        doc,
        rendered,
    }
}

#[tokio::test]
async fn scratchpad_write_builds_the_disciplined_document_and_projects_the_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The handler assembles the document from the flat tool fields and forwards an omitted revision
    // as a create — the request matches only if it did exactly that.
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ScratchpadWrite {
            name,
            doc,
            expected_revision: None,
        } if name == "plan" && doc == sample_doc() => {
            Ok(IpcResponse::Scratchpad(sample_scratchpad("plan")))
        }
        _ => Err(IpcError::Internal("unexpected write".into())),
    });

    let result = handler(socket)
        .scratchpad_write(Parameters(ScratchpadWriteArg {
            name: "plan".into(),
            objective: "Ship v1".into(),
            context: "RC cut".into(),
            plan: vec!["Cut RC".into()],
            acceptance_criteria: vec!["soak green".into()],
            risks: vec!["none identified".into()],
            status: "in progress".into(),
            notes: None,
            expected_revision: None,
        }))
        .await
        .expect("scratchpad_write succeeds");
    let back: ScratchpadView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.name, "plan");
    assert_eq!(back.revision, 1);
    assert!(back.rendered.starts_with("# plan"));
}

#[tokio::test]
async fn scratchpad_read_projects_the_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ScratchpadRead { name } if name == "plan" => {
            Ok(IpcResponse::Scratchpad(sample_scratchpad("plan")))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .scratchpad_read(Parameters(ScratchpadNameArg {
            name: "plan".into(),
        }))
        .await
        .expect("scratchpad_read succeeds");
    let back: ScratchpadView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.doc, sample_doc());
}

#[tokio::test]
async fn scratchpad_read_resolves_a_solo_link() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // A solo:// link routes to the scope-checked resolver (not a bare-name read); the resolved
    // scratchpad is projected just like a direct read.
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ResolveLink { link } if link == "solo://proj/1/scratchpad/1" => Ok(
            IpcResponse::Link(LinkContent::Scratchpad(sample_scratchpad("plan"))),
        ),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .scratchpad_read(Parameters(ScratchpadNameArg {
            name: "solo://proj/1/scratchpad/1".into(),
        }))
        .await
        .expect("scratchpad_read resolves the link");
    let back: ScratchpadView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.name, "plan");
}

#[tokio::test]
async fn todo_get_resolves_a_solo_link() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ResolveLink { link } if link == "solo://proj/1/todo/7" => {
            Ok(IpcResponse::Link(LinkContent::Todo(sample_todo(7))))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_get(Parameters(TodoGetArg {
            todo: TodoRef::Link("solo://proj/1/todo/7".into()),
        }))
        .await
        .expect("todo_get resolves the link");
    let back: TodoView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.id, TodoId::from_raw(7));
}

#[tokio::test]
async fn todo_get_reads_a_bare_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // A numeric id still routes to the plain get — the link form does not change the id path.
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TodoGet { todo } if todo == TodoId::from_raw(7) => {
            Ok(IpcResponse::Todo(sample_todo(7)))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_get(Parameters(TodoGetArg {
            todo: TodoRef::Id(7),
        }))
        .await
        .expect("todo_get reads the id");
    let back: TodoView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.id, TodoId::from_raw(7));
}

#[tokio::test]
async fn scratchpad_list_projects_the_summaries() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let summary = ScratchpadSummary {
        id: ScratchpadId::from_raw(1),
        name: "plan".into(),
        tags: vec!["release".into()],
        archived: false,
        revision: 2,
        objective: "Ship v1".into(),
    };
    let canned = summary.clone();
    spawn_fake_app(socket.clone(), move |request| match request {
        IpcRequest::ScratchpadList => Ok(IpcResponse::Scratchpads(vec![canned.clone()])),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .scratchpad_list()
        .await
        .expect("scratchpad_list succeeds");
    let back: Vec<ScratchpadSummary> =
        serde_json::from_value(structured_of(result)["scratchpads"].clone()).expect("decode list");
    assert_eq!(back, vec![summary]);
}

#[tokio::test]
async fn scratchpad_add_tags_threads_its_arguments_and_projects_the_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ScratchpadAddTags { name, tags } if name == "plan" && tags == ["p1"] => {
            Ok(IpcResponse::Scratchpad(sample_scratchpad("plan")))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .scratchpad_add_tags(Parameters(ScratchpadTagsArg {
            name: "plan".into(),
            tags: vec!["p1".into()],
        }))
        .await
        .expect("scratchpad_add_tags succeeds");
    let back: ScratchpadView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.name, "plan");
}

#[tokio::test]
async fn scratchpad_archive_threads_the_flag_through() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ScratchpadArchive {
            name,
            archived: true,
        } if name == "plan" => Ok(IpcResponse::Scratchpad(sample_scratchpad("plan"))),
        _ => Err(IpcError::Internal("expected an archive".into())),
    });

    handler(socket)
        .scratchpad_archive(Parameters(ScratchpadArchiveArg {
            name: "plan".into(),
            archived: true,
        }))
        .await
        .expect("scratchpad_archive succeeds");
}

#[tokio::test]
async fn scratchpad_delete_reports_whether_it_was_removed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::ScratchpadDelete { name } if name == "plan" => {
            Ok(IpcResponse::ScratchpadDeleted(true))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .scratchpad_delete(Parameters(ScratchpadNameArg {
            name: "plan".into(),
        }))
        .await
        .expect("scratchpad_delete succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "deleted": true })
    );
}

/// A sample disciplined document for the todo response-projection tests.
fn sample_todo_doc() -> TodoDoc {
    TodoDoc {
        title: "ship".into(),
        description: "cut the release".into(),
        acceptance_criteria: vec!["soak green".into()],
        risks: vec!["none identified".into()],
        status: TodoStatus::Open,
    }
}

/// A sample todo view for the response-projection tests.
fn sample_todo(id: u64) -> TodoView {
    TodoView {
        id: TodoId::from_raw(id),
        doc: sample_todo_doc(),
        tags: vec!["release".into()],
        blockers: Vec::new(),
        blocked_by: Vec::new(),
        blocked: false,
        comments: Vec::new(),
        locked_by: None,
        revision: 1,
    }
}

#[tokio::test]
async fn todo_create_builds_the_disciplined_document_and_projects_the_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The handler assembles the document from the flat tool fields and maps the wire status.
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TodoCreate { doc } if doc == sample_todo_doc() => {
            Ok(IpcResponse::Todo(sample_todo(5)))
        }
        _ => Err(IpcError::Internal("unexpected create".into())),
    });

    let result = handler(socket)
        .todo_create(Parameters(TodoCreateArg {
            title: "ship".into(),
            description: "cut the release".into(),
            acceptance_criteria: vec!["soak green".into()],
            risks: vec!["none identified".into()],
            status: TodoStatusArg::Open,
        }))
        .await
        .expect("todo_create succeeds");
    let back: TodoView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.id, TodoId::from_raw(5));
    assert_eq!(back.doc, sample_todo_doc());
}

#[tokio::test]
async fn todo_list_projects_the_summaries() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let summary = TodoSummary {
        id: TodoId::from_raw(3),
        title: "ship".into(),
        status: TodoStatus::InProgress,
        tags: vec!["release".into()],
        blocked: true,
        locked_by: Some(ProcessId::from_raw(9)),
        revision: 2,
    };
    let canned = summary.clone();
    spawn_fake_app(socket.clone(), move |request| match request {
        IpcRequest::TodoList => Ok(IpcResponse::Todos(vec![canned.clone()])),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_list()
        .await
        .expect("todo_list succeeds");
    let back: Vec<TodoSummary> =
        serde_json::from_value(structured_of(result)["todos"].clone()).expect("decode list");
    assert_eq!(back, vec![summary]);
}

#[tokio::test]
async fn todo_complete_threads_the_id_and_projects_the_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TodoComplete { todo } if todo == TodoId::from_raw(7) => {
            Ok(IpcResponse::Todo(sample_todo(7)))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_complete(Parameters(TodoArg { todo: 7 }))
        .await
        .expect("todo_complete succeeds");
    let back: TodoView = serde_json::from_value(structured_of(result)).expect("decode view");
    assert_eq!(back.id, TodoId::from_raw(7));
}

#[tokio::test]
async fn a_blocked_completion_becomes_a_tool_execution_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The core refused completion because the todo is gated; the agent must see it as an actionable
    // tool error naming the blockers, so it can finish those first.
    spawn_fake_app(socket.clone(), |_request| {
        Err(IpcError::TodoBlocked {
            by: vec![TodoId::from_raw(2)],
        })
    });

    let result = handler(socket)
        .todo_complete(Parameters(TodoArg { todo: 9 }))
        .await
        .expect("a refusal is a tool result, not a protocol error");
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn todo_comment_create_projects_the_todo_and_comment_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TodoCommentCreate { todo, body }
            if todo == TodoId::from_raw(4) && body == "note" =>
        {
            Ok(IpcResponse::TodoComment {
                todo: sample_todo(4),
                comment: 1,
            })
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_comment_create(Parameters(TodoCommentCreateArg {
            todo: 4,
            body: "note".into(),
        }))
        .await
        .expect("todo_comment_create succeeds");
    let structured = structured_of(result);
    assert_eq!(structured["comment"], serde_json::json!(1));
    let back: TodoView = serde_json::from_value(structured["todo"].clone()).expect("decode todo");
    assert_eq!(back.id, TodoId::from_raw(4));
}

#[tokio::test]
async fn todo_delete_reports_whether_it_was_removed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TodoDelete { todo } if todo == TodoId::from_raw(5) => {
            Ok(IpcResponse::TodoDeleted(true))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_delete(Parameters(TodoArg { todo: 5 }))
        .await
        .expect("todo_delete succeeds");
    assert_eq!(
        structured_of(result),
        serde_json::json!({ "deleted": true })
    );
}

/// The `Comment` core type is part of the todo wire surface (the comment-list reply); this guards
/// that it serializes to the shape the agent reads.
#[tokio::test]
async fn todo_comment_list_projects_the_comments() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::TodoCommentList { todo } if todo == TodoId::from_raw(4) => {
            Ok(IpcResponse::TodoComments(vec![Comment {
                id: 1,
                body: "looks good".into(),
                author: None,
            }]))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .todo_comment_list(Parameters(TodoArg { todo: 4 }))
        .await
        .expect("todo_comment_list succeeds");
    let back: Vec<Comment> =
        serde_json::from_value(structured_of(result)["comments"].clone()).expect("decode comments");
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].body, "looks good");
}

/// `help` answers from the core's embedded guide with no app round-trip — nothing listens on
/// the socket here, and it still succeeds. That is deliberate: it must work when Soloist is
/// down, which is when an agent most needs it.
#[tokio::test]
async fn help_returns_the_guide_without_the_app() {
    let handler = handler(PathBuf::from("nothing-listens-here.sock"));

    let result = handler.help().await.expect("help succeeds with no app");
    let help = structured_of(result)["help"]
        .as_str()
        .expect("a help string")
        .to_owned();
    assert!(help.contains("bind_session_process"));
    assert!(help.contains("lock_acquire"));
}

#[tokio::test]
async fn submit_solo_feedback_threads_the_message_and_projects_the_entry() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SubmitFeedback { message } if message == "love the log search" => {
            Ok(IpcResponse::Feedback(FeedbackEntry {
                id: 3,
                message,
                submitted_unix_millis: 1_700_000_000_000,
            }))
        }
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .submit_solo_feedback(Parameters(SubmitFeedbackArg {
            message: "love the log search".into(),
        }))
        .await
        .expect("submit_solo_feedback succeeds");
    let back: FeedbackEntry =
        serde_json::from_value(structured_of(result)).expect("decode the entry");
    assert_eq!(back.id, 3);
    assert_eq!(back.message, "love the log search");
}

#[tokio::test]
async fn setup_agent_integration_defaults_to_agents_md() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SetupAgentIntegration {
            file: IntegrationFile::AgentsMd,
        } => Ok(IpcResponse::IntegrationWritten(IntegrationWrite {
            path: PathBuf::from("/p/AGENTS.md"),
            created: true,
        })),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .setup_agent_integration(Parameters(SetupAgentIntegrationArg { file: None }))
        .await
        .expect("setup_agent_integration succeeds");
    let back: IntegrationWrite =
        serde_json::from_value(structured_of(result)).expect("decode the write");
    assert!(back.created);
    assert_eq!(back.path, PathBuf::from("/p/AGENTS.md"));
}

#[tokio::test]
async fn setup_agent_integration_threads_an_explicit_claude_md() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::SetupAgentIntegration {
            file: IntegrationFile::ClaudeMd,
        } => Ok(IpcResponse::IntegrationWritten(IntegrationWrite {
            path: PathBuf::from("/p/CLAUDE.md"),
            created: false,
        })),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .setup_agent_integration(Parameters(SetupAgentIntegrationArg {
            file: Some(IntegrationFileArg::ClaudeMd),
        }))
        .await
        .expect("setup_agent_integration succeeds");
    let back: IntegrationWrite =
        serde_json::from_value(structured_of(result)).expect("decode the write");
    assert!(!back.created);
    assert_eq!(back.path, PathBuf::from("/p/CLAUDE.md"));
}

#[tokio::test]
async fn prompt_template_create_threads_the_scope_and_projects_the_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::PromptTemplateCreate {
            scope: PromptScope::Global,
            name,
            description,
            body,
        } if name == "review" => Ok(IpcResponse::PromptTemplate(PromptTemplateView {
            id: PromptTemplateId::from_raw(4),
            name,
            description,
            placeholders: vec!["diff".into()],
            body,
            scope: PromptScope::Global,
            revision: 1,
        })),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .prompt_template_create(Parameters(PromptTemplateCreateArg {
            name: "review".into(),
            description: Some("PR review".into()),
            body: "Review {{diff}}".into(),
            scope: Some(PromptScopeArg::Global),
        }))
        .await
        .expect("prompt_template_create succeeds");
    let back: PromptTemplateView =
        serde_json::from_value(structured_of(result)).expect("decode the view");
    assert_eq!(back.placeholders, vec!["diff".to_owned()]);
    assert_eq!(back.scope, PromptScope::Global);
}

/// An omitted scope addresses the effective project, and a stale update surfaces as a
/// tool-execution error the model can read and retry on.
#[tokio::test]
async fn a_stale_prompt_template_update_becomes_a_tool_execution_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |request| match request {
        IpcRequest::PromptTemplateUpdate {
            scope: PromptScope::Project,
            expected_revision: 1,
            ..
        } => Err(IpcError::PromptTemplateRevisionConflict {
            expected: Some(1),
            actual: Some(2),
        }),
        _ => Err(IpcError::Internal("unexpected request".into())),
    });

    let result = handler(socket)
        .prompt_template_update(Parameters(PromptTemplateUpdateArg {
            name: "review".into(),
            description: None,
            body: "new body".into(),
            expected_revision: 1,
            scope: None,
        }))
        .await
        .expect("a request error is a tool result, not a protocol error");
    assert_eq!(result.is_error, Some(true));
}
