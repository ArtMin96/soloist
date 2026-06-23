use super::*;
use soloist_core::{
    AgentKind, AgentTool, Origin, ProcStatus, ProcessId, ProcessKind, ProcessView, ProjectId,
    ProjectView, PromptMode, Readiness, SessionId, StartSummary, Whoami,
};
use std::path::PathBuf;

#[test]
fn requests_round_trip_through_json() {
    let requests = [
        IpcRequest::Whoami,
        IpcRequest::BindSessionProcess {
            process: ProcessId::from_raw(1),
        },
        IpcRequest::RegisterAgent {
            label: "claude-code".into(),
        },
        IpcRequest::SelectProject {
            project: ProjectId::from_raw(2),
        },
        IpcRequest::ListProjects,
        IpcRequest::GetProjectStatus {
            project: Some(ProjectId::from_raw(3)),
        },
        IpcRequest::GetProjectStatus { project: None },
        IpcRequest::ListProcesses,
        IpcRequest::GetProcessStatus {
            process: ProcessId::from_raw(4),
        },
        IpcRequest::StartProcess {
            process: ProcessId::from_raw(5),
        },
        IpcRequest::StopProcess {
            process: ProcessId::from_raw(6),
        },
        IpcRequest::RestartProcess {
            process: ProcessId::from_raw(7),
        },
        IpcRequest::SendInput {
            process: ProcessId::from_raw(8),
            input: "ls\r".into(),
            wait_ms: Some(200),
        },
        IpcRequest::SendInput {
            process: ProcessId::from_raw(9),
            input: "\u{3}".into(),
            wait_ms: None,
        },
        IpcRequest::SpawnAgent {
            tool: "Claude".into(),
            extra_args: vec!["--model".into(), "opus".into()],
        },
        IpcRequest::ListAgentTools,
        IpcRequest::StartAllCommands,
        IpcRequest::StopAllCommands,
        IpcRequest::RestartAllCommands,
        IpcRequest::GetProcessOutput {
            process: ProcessId::from_raw(10),
            lines: Some(50),
        },
        IpcRequest::GetProcessRawOutput {
            process: ProcessId::from_raw(11),
        },
        IpcRequest::SearchOutput {
            process: ProcessId::from_raw(12),
            query: "error".into(),
            limit: Some(10),
        },
        IpcRequest::SearchRawOutput {
            process: ProcessId::from_raw(13),
            query: "warn".into(),
            limit: None,
        },
        IpcRequest::ClearOutput {
            process: ProcessId::from_raw(14),
        },
        IpcRequest::FlushTerminalPerf {
            process: ProcessId::from_raw(15),
        },
        IpcRequest::GetProcessPorts {
            process: ProcessId::from_raw(16),
        },
        IpcRequest::ServicesList,
        IpcRequest::WaitForBoundPort {
            process: ProcessId::from_raw(17),
            port: 3000,
            timeout_ms: Some(5000),
        },
        IpcRequest::WaitForBoundPort {
            process: ProcessId::from_raw(18),
            port: 8080,
            timeout_ms: None,
        },
    ];
    for request in requests {
        let json = serde_json::to_string(&request).expect("serialize");
        let back: IpcRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, request);
    }
}

#[test]
fn every_response_variant_round_trips_through_json() {
    let view = ProcessView {
        id: ProcessId::from_raw(7),
        project: ProjectId::from_raw(1),
        kind: ProcessKind::Terminal,
        label: "term".into(),
        status: ProcStatus::Running,
        exit_code: None,
        requires_trust: false,
        ports: Vec::new(),
        ready: Readiness::Ungated,
    };
    let summary = ProjectSummary {
        id: ProjectId::from_raw(1),
        name: "storefront".into(),
        root: PathBuf::from("/projects/storefront"),
    };
    // The list variants wrap a sequence; this is what a purely internal tag could not
    // serialize, so exercising every variant guards the response envelope's tagging.
    let responses = [
        IpcResponse::Whoami(Whoami {
            session: SessionId::from_raw(1),
            origin: Origin::Unbound,
            bound_process: None,
            effective_project: None,
        }),
        IpcResponse::Acked,
        IpcResponse::Projects(vec![summary.clone()]),
        IpcResponse::ProjectStatus(ProjectStatus {
            project: summary.clone(),
            processes: vec![view.clone()],
        }),
        IpcResponse::Processes(vec![view.clone()]),
        IpcResponse::Process(view.clone()),
        IpcResponse::Stopped(true),
        IpcResponse::InputSent(Some("$ ls\nfile.txt".into())),
        IpcResponse::InputSent(None),
        IpcResponse::Spawned(ProcessId::from_raw(12)),
        IpcResponse::AgentTools(vec![AgentTool {
            name: "Claude".into(),
            command: "claude".into(),
            default_args: Vec::new(),
            kind: AgentKind::Claude,
            prompt_mode: PromptMode::AppendedArg,
        }]),
        IpcResponse::BulkStarted(StartSummary {
            started: vec![ProcessId::from_raw(3), ProcessId::from_raw(4)],
            skipped_untrusted: vec![ProcessId::from_raw(5)],
        }),
        IpcResponse::BulkStopped(2),
        IpcResponse::Lines(vec!["error: boom".into(), "error: bang".into()]),
        IpcResponse::RawOutput("\u{1b}[31merror\u{1b}[0m".into()),
        IpcResponse::Ports(vec![3000, 8080]),
        IpcResponse::PortWait(PortWaitOutcome::Bound),
    ];
    for response in responses {
        let json = serde_json::to_string(&response).expect("serialize");
        let back: IpcResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, response);
    }
}

#[test]
fn every_port_wait_outcome_round_trips() {
    for outcome in [
        PortWaitOutcome::Bound,
        PortWaitOutcome::TimedOut,
        PortWaitOutcome::NotRunning,
    ] {
        let json = serde_json::to_string(&outcome).expect("serialize");
        let back: PortWaitOutcome = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, outcome);
    }
}

#[test]
fn a_typed_error_round_trips() {
    for error in [
        IpcError::UnknownProcess,
        IpcError::UnknownProject,
        IpcError::ForeignProcess,
        IpcError::ForeignProject,
        IpcError::NoProjectScope,
        IpcError::OutOfScope,
        IpcError::Untrusted,
        IpcError::UnknownTool,
        IpcError::Internal("disk full".into()),
    ] {
        let json = serde_json::to_string(&error).expect("serialize");
        let back: IpcError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, error);
    }
}

#[test]
fn request_errors_are_distinguished_from_server_errors() {
    // The one classifier every adapter reuses: a request-caused refusal is actionable
    // feedback (MCP `isError: true`, HTTP 4xx); a server failure is not (protocol error, 5xx).
    for error in [
        IpcError::UnknownProcess,
        IpcError::UnknownProject,
        IpcError::ForeignProcess,
        IpcError::ForeignProject,
        IpcError::NoProjectScope,
        IpcError::OutOfScope,
        IpcError::Untrusted,
        IpcError::UnknownTool,
    ] {
        assert!(error.is_request_error(), "{error} is request-caused");
    }
    assert!(
        !IpcError::Internal("disk full".into()).is_request_error(),
        "a server failure is not request-caused"
    );
}

#[test]
fn core_action_errors_map_to_the_wire_error() {
    use soloist_core::ScopedActionError;
    // The single place core action errors become wire errors, so every adapter agrees.
    assert_eq!(
        IpcError::from(ScopedActionError::UnknownProcess),
        IpcError::UnknownProcess
    );
    assert_eq!(
        IpcError::from(ScopedActionError::NoProjectScope),
        IpcError::NoProjectScope
    );
    assert_eq!(
        IpcError::from(ScopedActionError::OutOfScope),
        IpcError::OutOfScope
    );
    assert_eq!(
        IpcError::from(ScopedActionError::Untrusted),
        IpcError::Untrusted
    );
}

#[test]
fn core_spawn_errors_map_to_the_wire_error() {
    use soloist_core::{LaunchAgentError, SpawnAgentError};
    assert_eq!(
        IpcError::from(SpawnAgentError::NoProjectScope),
        IpcError::NoProjectScope
    );
    assert_eq!(
        IpcError::from(SpawnAgentError::Launch(LaunchAgentError::UnknownTool)),
        IpcError::UnknownTool
    );
    assert_eq!(
        IpcError::from(LaunchAgentError::UnknownProject),
        IpcError::UnknownProject
    );
}

#[test]
fn core_identity_errors_map_to_the_wire_error() {
    use soloist_core::IdentityError;
    assert_eq!(
        IpcError::from(IdentityError::UnknownProcess),
        IpcError::UnknownProcess
    );
    assert_eq!(
        IpcError::from(IdentityError::ForeignProcess),
        IpcError::ForeignProcess
    );
    assert_eq!(
        IpcError::from(IdentityError::UnknownProject),
        IpcError::UnknownProject
    );
    assert_eq!(
        IpcError::from(IdentityError::ForeignProject),
        IpcError::ForeignProject
    );
}

#[test]
fn a_project_summary_drops_the_ui_icon() {
    // The agent-facing projection keeps identity and root but never the icon data-URL.
    let view = ProjectView {
        id: ProjectId::from_raw(5),
        name: "storefront".into(),
        root: PathBuf::from("/projects/storefront"),
        icon: Some("data:image/png;base64,AAAA".into()),
    };
    let summary = ProjectSummary::from_view(&view);
    assert_eq!(summary.id, view.id);
    assert_eq!(summary.name, view.name);
    assert_eq!(summary.root, view.root);

    // The serialized shape carries no icon field at all.
    let json = serde_json::to_string(&summary).expect("serialize");
    assert!(!json.contains("icon"), "summary must not ship the UI icon");
}
