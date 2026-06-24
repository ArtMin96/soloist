use super::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use soloist_core::{
    AcquireOutcome, AgentKind, AgentTool, FireCond, LeaseView, Origin, ProcStatus, ProcessId,
    ProcessKind, ProcessView, ProjectId, PromptMode, Readiness, ScratchpadDoc, ScratchpadId,
    ScratchpadSummary, ScratchpadView, SessionId, SetWhenIdleOutcome, StartSummary, TimerId,
    TimerStatus, TimerView, Whoami,
};
use soloist_ipc::{
    read_frame, write_frame, IpcError, IpcRequest, IpcResponse, IpcResult, PortWaitOutcome,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use tokio::net::UnixListener;

use crate::args::{
    LockAcquireArg, LockKeyArg, OutputArg, ProcessArg, RenameArg, ScratchpadArchiveArg,
    ScratchpadNameArg, ScratchpadTagsArg, ScratchpadWriteArg, SearchArg, SelectProjectArg,
    SendInputArg, SpawnAgentArg, TimerArg, TimerFireWhenIdleArg, TimerSetArg, WaitForPortArg,
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

/// A handler whose single client connection talks to the fake app on `socket`.
fn handler(socket: PathBuf) -> SoloistMcp {
    SoloistMcp::new(Arc::new(AppClient::new(None, socket)))
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
];

/// The router [`SoloistMcp::new`] composes from the per-category sub-routers must serve exactly
/// the intended surface. This guards the split itself: a category sub-router left out of the
/// `+` composition, a tool name colliding across two category files (`ToolRouter::add_route` is
/// a map insert that silently overwrites), or an accidental add/rename all change what the
/// served router reports — drift the per-tool behaviour tests cannot catch, because they invoke
/// the tool methods directly without going through the composed router.
#[test]
fn served_router_exposes_exactly_the_expected_tool_surface() {
    let dir = tempfile::tempdir().expect("temp dir");
    // `list_all` reads the statically composed router; no IPC connection is opened, so the
    // socket path is never touched.
    let served: BTreeSet<String> = handler(dir.path().join("soloist-ipc.sock"))
        .tool_router
        .list_all()
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();

    let expected: BTreeSet<String> = EXPECTED_TOOL_SURFACE
        .iter()
        .map(|name| name.to_string())
        .collect();

    assert_eq!(
        served, expected,
        "the served MCP tool surface drifted from the intended set"
    );
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
        body: "resume work".into(),
        fire: FireCond::At,
        status: TimerStatus::Armed,
        deadline_unix_millis: 1_700_000_005_000,
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
                    body,
                    fire: FireCond::WhenIdleAll {
                        watched: processes.clone(),
                    },
                    status: TimerStatus::Armed,
                    deadline_unix_millis: 0,
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
