use super::*;
use soloist_core::{
    AgentKind, AgentTool, Origin, ProcStatus, ProcessKind, ProcessView, PromptMode, Readiness,
    SessionId, Whoami,
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
async fn a_typed_app_error_becomes_a_tool_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    spawn_fake_app(socket.clone(), |_request| Err(IpcError::UnknownProcess));

    let result = handler(socket)
        .get_process_status(Parameters(ProcessArg { process: 99 }))
        .await;
    assert!(result.is_err(), "an app error must surface as a tool error");
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
async fn a_refused_action_surfaces_as_a_tool_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    // The core refused the action (untrusted / out of scope); the agent must see an error.
    spawn_fake_app(socket.clone(), |_request| Err(IpcError::Untrusted));

    let result = handler(socket)
        .start_process(Parameters(ProcessArg { process: 5 }))
        .await;
    assert!(
        result.is_err(),
        "a refused action must surface as a tool error"
    );
}
