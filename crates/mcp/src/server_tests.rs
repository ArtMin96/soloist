use super::*;
use soloist_core::{
    AgentKind, AgentTool, Origin, ProcStatus, ProcessKind, ProcessView, PromptMode, Readiness,
    SessionId, StartSummary, Whoami,
};
use soloist_ipc::{read_frame, write_frame, IpcError, IpcResult};
use std::path::PathBuf;
use tokio::net::UnixListener;

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
