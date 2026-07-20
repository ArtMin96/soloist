use super::*;

use crate::error::IpcError;
use soloist_core::{
    AcquireOutcome, AgentKind, AgentTool, ExportedTemplate, FeedbackEntry, FireCond,
    IntegrationFile, IntegrationWrite, LeaseView, MissingPolicy, Origin, ProcStatus, ProcessId,
    ProcessKind, ProcessView, ProjectId, ProjectRef, ProjectView, PromptMode, Readiness,
    RenderedPrompt, ScratchpadId, ScratchpadView, SessionId, SetWhenIdleOutcome, StartSummary,
    TemplateId, TemplateKind, TemplateScope, TemplateSummary, TemplateView, TimerId, TimerStatus,
    TimerView, TodoDoc, TodoId, TodoStatus, TodoView, Whoami,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A sample scratchpad view for the create-with-seed response round-trip.
fn sample_scratchpad() -> ScratchpadView {
    ScratchpadView {
        id: ScratchpadId::from_raw(1),
        name: "today".into(),
        tags: vec!["release".into()],
        archived: false,
        revision: 1,
        body: "## Plan".into(),
        rendered: "# today\n\n## Plan\n".into(),
    }
}

/// A sample todo view for the create-with-seed response round-trip.
fn sample_todo() -> TodoView {
    TodoView {
        id: TodoId::from_raw(1),
        doc: TodoDoc {
            title: "sweep".into(),
            body: "## Steps".into(),
            status: TodoStatus::Open,
        },
        tags: Vec::new(),
        blockers: Vec::new(),
        blocked_by: Vec::new(),
        blocked: false,
        comments: Vec::new(),
        locked_by: None,
        scratchpad: None,
        revision: 1,
    }
}

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
        IpcRequest::SelectProcess {
            process: ProcessId::from_raw(19),
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
        IpcRequest::RenameProcess {
            process: ProcessId::from_raw(20),
            label: "worker".into(),
        },
        IpcRequest::CloseProcess {
            process: ProcessId::from_raw(21),
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
        IpcRequest::LockAcquire {
            key: "deploy".into(),
            ttl_ms: Some(30_000),
        },
        IpcRequest::LockAcquire {
            key: "deploy".into(),
            ttl_ms: None,
        },
        IpcRequest::LockStatus {
            key: "deploy".into(),
        },
        IpcRequest::LockRelease {
            key: "deploy".into(),
        },
        IpcRequest::TimerSet {
            body: "ping".into(),
            after_ms: Some(5_000),
        },
        IpcRequest::TimerSet {
            body: "now".into(),
            after_ms: None,
        },
        IpcRequest::TimerFireWhenIdleAny {
            body: "any".into(),
            processes: vec![ProcessId::from_raw(2)],
            max_wait_ms: Some(60_000),
        },
        IpcRequest::TimerFireWhenIdleAll {
            body: "all".into(),
            processes: vec![ProcessId::from_raw(2), ProcessId::from_raw(3)],
            max_wait_ms: None,
        },
        IpcRequest::TimerCancel {
            timer: TimerId::from_raw(1),
        },
        IpcRequest::TimerPause {
            timer: TimerId::from_raw(1),
        },
        IpcRequest::TimerResume {
            timer: TimerId::from_raw(1),
        },
        IpcRequest::TimerList,
        IpcRequest::SubmitFeedback {
            message: "the sidebar flickers".into(),
        },
        IpcRequest::SetupAgentIntegration {
            file: IntegrationFile::ClaudeMd,
        },
        IpcRequest::SetupAgentIntegration {
            file: IntegrationFile::AgentsMd,
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
        resumable: false,
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
            origin: Origin::Process(ProcessId::from_raw(7)),
            // Populated so the enriched process and project projections round-trip in the envelope.
            bound_process: Some(view.clone()),
            selected_process: None,
            effective_project: Some(ProjectRef {
                id: ProjectId::from_raw(1),
                name: Some("storefront".into()),
            }),
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
        IpcResponse::LeaseOutcome(AcquireOutcome::Acquired(LeaseView {
            key: "deploy".into(),
            owner: ProcessId::from_raw(7),
            expires_unix_millis: 1_700_000_030_000,
        })),
        IpcResponse::LeaseOutcome(AcquireOutcome::Held(LeaseView {
            key: "deploy".into(),
            owner: ProcessId::from_raw(8),
            expires_unix_millis: 1_700_000_030_000,
        })),
        IpcResponse::LeaseStatus(Some(LeaseView {
            key: "deploy".into(),
            owner: ProcessId::from_raw(7),
            expires_unix_millis: 1_700_000_030_000,
        })),
        IpcResponse::LeaseStatus(None),
        IpcResponse::LeaseReleased(true),
        IpcResponse::TimerArmed(TimerView {
            id: TimerId::from_raw(3),
            owner: ProcessId::from_raw(1),
            body: "ping".into(),
            fire: FireCond::At,
            status: TimerStatus::Armed,
            deadline_unix_millis: 1_700_000_005_000,
            waiting_on: vec![],
            already_idle: false,
            paused_remaining_millis: None,
        }),
        IpcResponse::TimerWhenIdle(SetWhenIdleOutcome {
            timer: TimerView {
                id: TimerId::from_raw(4),
                owner: ProcessId::from_raw(1),
                body: "all done".into(),
                fire: FireCond::WhenIdleAll {
                    watched: vec![ProcessId::from_raw(2), ProcessId::from_raw(3)],
                },
                status: TimerStatus::Armed,
                deadline_unix_millis: 1_700_000_060_000,
                waiting_on: vec![ProcessId::from_raw(2), ProcessId::from_raw(3)],
                already_idle: false,
                paused_remaining_millis: None,
            },
            already_idle: false,
            waiting_on: vec![ProcessId::from_raw(2), ProcessId::from_raw(3)],
        }),
        IpcResponse::TimerChanged(true),
        IpcResponse::Timers(vec![TimerView {
            id: TimerId::from_raw(5),
            owner: ProcessId::from_raw(1),
            body: "paused".into(),
            fire: FireCond::WhenIdleAny {
                watched: vec![ProcessId::from_raw(9)],
            },
            status: TimerStatus::Paused,
            deadline_unix_millis: 0,
            waiting_on: vec![ProcessId::from_raw(9)],
            already_idle: false,
            paused_remaining_millis: Some(45_000),
        }]),
        IpcResponse::Feedback(FeedbackEntry {
            id: 7,
            message: "the sidebar flickers".into(),
            submitted_unix_millis: 1_700_000_000_000,
        }),
        IpcResponse::IntegrationWritten(IntegrationWrite {
            path: PathBuf::from("/projects/storefront/CLAUDE.md"),
            created: true,
        }),
        IpcResponse::ScratchpadWritten {
            scratchpad: sample_scratchpad(),
            seeded_from: Some("daily".into()),
        },
        IpcResponse::TodoCreated {
            todo: sample_todo(),
            seeded_from: None,
        },
    ];
    for response in responses {
        let json = serde_json::to_string(&response).expect("serialize");
        let back: IpcResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, response);
    }
}

/// Asserts `value` serializes to exactly `json` and deserializes back from it.
fn pins<T>(value: T, json: serde_json::Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(serde_json::to_value(&value).expect("serialize"), json);
    assert_eq!(
        serde_json::from_value::<T>(json).expect("deserialize"),
        value
    );
}

/// The prompt-template requests are read off the wire by an out-of-process client, so each `op` tag
/// and field name is the contract rather than merely the fact that it round-trips: a symmetric
/// to_string→from_str would agree with itself through a rename that silently breaks every reader.
#[test]
fn the_prompt_template_requests_serialize_to_their_wire_shape() {
    pins(
        IpcRequest::PromptTemplateList { scope: None },
        serde_json::json!({ "op": "prompt_template_list", "scope": null }),
    );
    pins(
        IpcRequest::PromptTemplateList {
            scope: Some(TemplateScope::Global),
        },
        serde_json::json!({ "op": "prompt_template_list", "scope": "global" }),
    );
    pins(
        IpcRequest::PromptTemplateRead {
            scope: TemplateScope::Project,
            name: "review".into(),
        },
        serde_json::json!({ "op": "prompt_template_read", "scope": "project", "name": "review" }),
    );
    pins(
        IpcRequest::PromptTemplateCreate {
            scope: TemplateScope::Global,
            name: "review".into(),
            description: Some("PR review".into()),
            body: "Review {{diff}}".into(),
        },
        serde_json::json!({
            "op": "prompt_template_create",
            "scope": "global",
            "name": "review",
            "description": "PR review",
            "body": "Review {{diff}}",
        }),
    );
    // An omitted description is carried as an explicit null, not a missing key: the core reads the
    // two apart (keep the stored description versus clear it), so the field must survive the wire.
    pins(
        IpcRequest::PromptTemplateUpdate {
            scope: TemplateScope::Project,
            name: "review".into(),
            description: None,
            body: "Review {{diff}} for {{focus}}".into(),
            expected_revision: 2,
        },
        serde_json::json!({
            "op": "prompt_template_update",
            "scope": "project",
            "name": "review",
            "description": null,
            "body": "Review {{diff}} for {{focus}}",
            "expected_revision": 2,
        }),
    );
    pins(
        IpcRequest::PromptTemplateDelete {
            scope: TemplateScope::Project,
            name: "review".into(),
        },
        serde_json::json!({ "op": "prompt_template_delete", "scope": "project", "name": "review" }),
    );
    pins(
        IpcRequest::PromptTemplateExport {
            scope: TemplateScope::Global,
            name: "review".into(),
        },
        serde_json::json!({ "op": "prompt_template_export", "scope": "global", "name": "review" }),
    );
}

/// The prompt-template replies, pinned for the same reason: the `ok` tag a client dispatches on, the
/// field names it reads, and — for an export — the format tag a saved envelope carries forever.
#[test]
fn the_prompt_template_responses_serialize_to_their_wire_shape() {
    pins(
        IpcResponse::PromptTemplate(TemplateView {
            id: TemplateId::from_raw(4),
            kind: TemplateKind::Prompt,
            name: "review".into(),
            description: Some("PR review".into()),
            body: "Review {{diff}}".into(),
            placeholders: vec!["diff".into()],
            scope: TemplateScope::Project,
            revision: 1,
        }),
        serde_json::json!({
            "ok": "prompt_template",
            "data": {
                "id": 4,
                "kind": "prompt",
                "name": "review",
                "description": "PR review",
                "body": "Review {{diff}}",
                "placeholders": ["diff"],
                "scope": "project",
                "revision": 1,
            },
        }),
    );
    // A summary is the same row minus the body — a client that listed and then rendered a body it
    // never received would show an empty template.
    pins(
        IpcResponse::PromptTemplates(vec![TemplateSummary {
            id: TemplateId::from_raw(4),
            kind: TemplateKind::Prompt,
            name: "review".into(),
            description: None,
            placeholders: vec!["diff".into()],
            scope: TemplateScope::Global,
            revision: 3,
        }]),
        serde_json::json!({
            "ok": "prompt_templates",
            "data": [{
                "id": 4,
                "kind": "prompt",
                "name": "review",
                "description": null,
                "placeholders": ["diff"],
                "scope": "global",
                "revision": 3,
            }],
        }),
    );
    // The purest case for pinning: the payload is a bare bool, so a round-trip would survive the
    // variant being renamed to anything at all.
    pins(
        IpcResponse::PromptTemplateDeleted(true),
        serde_json::json!({ "ok": "prompt_template_deleted", "data": true }),
    );
    pins(
        IpcResponse::PromptTemplateExport(ExportedTemplate {
            format: "soloist.prompt-template/v1".into(),
            name: "review".into(),
            description: None,
            body: "Review {{diff}}".into(),
        }),
        serde_json::json!({
            "ok": "prompt_template_export",
            "data": {
                "format": "soloist.prompt-template/v1",
                "name": "review",
                "description": null,
                "body": "Review {{diff}}",
            },
        }),
    );
}

/// The render exchange is read off the wire by an out-of-process client, so its exact shape is the
/// contract, not merely that it round-trips: the two op/ok tags, the values map as a plain JSON
/// object, and the policy as its snake_case tag. Pinned literally for the same reason the port-wait
/// tags below are — a symmetric round-trip would survive a rename that breaks every reader.
#[test]
fn the_prompt_render_exchange_serializes_to_its_wire_shape() {
    let request = IpcRequest::PromptTemplateRender {
        scope: TemplateScope::Project,
        name: "review".into(),
        values: BTreeMap::from([("diff".to_owned(), "a/b.rs".to_owned())]),
        policy: MissingPolicy::Strict,
    };
    let request_json = serde_json::json!({
        "op": "prompt_template_render",
        "scope": "project",
        "name": "review",
        "values": { "diff": "a/b.rs" },
        "policy": "strict",
    });
    assert_eq!(
        serde_json::to_value(&request).expect("serialize"),
        request_json
    );
    assert_eq!(
        serde_json::from_value::<IpcRequest>(request_json).expect("deserialize"),
        request
    );

    let response = IpcResponse::PromptTemplateRendered(RenderedPrompt {
        text: "Review a/b.rs for {{focus}}".into(),
        unfilled: vec!["focus".into()],
        unknown: vec!["dif".into()],
    });
    let response_json = serde_json::json!({
        "ok": "prompt_template_rendered",
        "data": {
            "text": "Review a/b.rs for {{focus}}",
            "unfilled": ["focus"],
            "unknown": ["dif"],
        },
    });
    assert_eq!(
        serde_json::to_value(&response).expect("serialize"),
        response_json
    );
    assert_eq!(
        serde_json::from_value::<IpcResponse>(response_json).expect("deserialize"),
        response
    );
}

#[test]
fn port_wait_outcomes_serialize_to_their_wire_tags() {
    // Out-of-process clients read these tags off the wire, so the exact string per variant is the
    // contract — a symmetric to_string→from_str round-trip would pass through a rename that silently
    // breaks every reader. Pin the literal tag (and that it deserializes back) instead.
    for (outcome, tag) in [
        (PortWaitOutcome::Bound, "\"bound\""),
        (PortWaitOutcome::TimedOut, "\"timed_out\""),
        (PortWaitOutcome::NotRunning, "\"not_running\""),
    ] {
        assert_eq!(serde_json::to_string(&outcome).expect("serialize"), tag);
        assert_eq!(
            serde_json::from_str::<PortWaitOutcome>(tag).expect("deserialize"),
            outcome
        );
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
        IpcError::NoBoundProcess,
        IpcError::OutOfScope,
        IpcError::Untrusted,
        IpcError::UnknownTool,
        IpcError::WorkerMayNotSpawn,
        IpcError::InvalidFeedback("feedback message is empty".into()),
        IpcError::UnmatchedIntegrationMarkers("AGENTS.md has unmatched markers".into()),
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
        IpcError::NoBoundProcess,
        IpcError::OutOfScope,
        IpcError::Untrusted,
        IpcError::UnknownTool,
        IpcError::WorkerMayNotSpawn,
        IpcError::InvalidFeedback("feedback message is empty".into()),
        IpcError::UnmatchedIntegrationMarkers("AGENTS.md has unmatched markers".into()),
    ] {
        assert!(error.is_request_error(), "{error} is request-caused");
    }
    assert!(
        !IpcError::Internal("disk full".into()).is_request_error(),
        "a server failure is not request-caused"
    );
}

#[test]
fn a_refused_integration_write_maps_to_its_own_wire_error() {
    use soloist_core::{IntegrationWriteError, SetupIntegrationError};

    let err = IpcError::from(SetupIntegrationError::Write(
        IntegrationWriteError::UnmatchedMarkers {
            path: "/p/AGENTS.md".into(),
        },
    ));
    assert!(matches!(err, IpcError::UnmatchedIntegrationMarkers(_)));
    assert!(err.is_request_error(), "the caller can fix the file");
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
        IpcError::from(SpawnAgentError::WorkerMayNotSpawn),
        IpcError::WorkerMayNotSpawn
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
fn core_coordination_errors_map_to_the_wire_error() {
    use soloist_core::CoordinationError;
    assert_eq!(
        IpcError::from(CoordinationError::NoProjectScope),
        IpcError::NoProjectScope
    );
    assert_eq!(
        IpcError::from(CoordinationError::NoBoundProcess),
        IpcError::NoBoundProcess
    );
    assert_eq!(
        IpcError::from(CoordinationError::PayloadTooLarge {
            what: "kv value",
            max_bytes: 4096,
        }),
        IpcError::PayloadTooLarge {
            what: "kv value".to_owned(),
            max_bytes: 4096,
        }
    );
    assert!(
        IpcError::PayloadTooLarge {
            what: "kv value".to_owned(),
            max_bytes: 4096,
        }
        .is_request_error(),
        "an oversized payload is the caller's to fix"
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
