use super::*;
use soloist_core::testing::{
    agent_registration, terminal_registration, FakeGitForge, FakeGitRepository, FakeLockRepo,
    FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTemplateRepo, FakeTrustRepo, GitChange,
};
use soloist_core::{
    AcquireOutcome, BranchOp, CorePorts, DomainEvent, IntegrationFile, McpFeatureGroup,
    MissingPolicy, NewPullRequest, Origin, PeerCredentials, ProcStatus, ProcessId, ProcessKind,
    ProcessSpec, ProjectRepo, Prompting, StartSummary, StashOp, SyncOp, TemplateScope, TokioClock,
    TrustRepo, TrustRequestOutcome, TrustRequestState,
};
use soloist_ipc::GitRefusal;
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// A façade over in-memory fakes — an alternate composition root, the same way the core's
/// own tests build one. Routing is what we exercise here; the behaviour behind each call
/// is tested in the core. Returned as an [`Arc`] because [`handle_request`] takes the façade by
/// shared handle (it clones it onto the blocking pool for the synchronous dispatch).
/// Serves one request nobody asked to be told about — which is every request here except the ones
/// that are about being told, and those call the real one with a receiver they keep.
async fn handle_request(
    facade: &Arc<Facade>,
    session: SessionId,
    request: IpcRequest,
) -> IpcResult {
    let (reports, _unheard) = mpsc::channel(1);
    super::handle_request(facade, session, request, reports).await
}

fn facade() -> Arc<Facade> {
    Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ))
}

async fn wait_for(rx: &mut broadcast::Receiver<DomainEvent>, target: ProcStatus) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { to, .. }) if to == target => return,
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

#[tokio::test]
async fn whoami_routes_to_the_identity_session() {
    let facade = facade();
    let session = grouped_session(&facade);
    match handle_request(&facade, session, IpcRequest::Whoami).await {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.session, session);
            assert!(who.bound_process.is_none());
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[tokio::test]
async fn register_agent_acks_and_whoami_reflects_the_label() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::RegisterAgent {
                label: "claude-code".into(),
            },
        )
        .await,
        Ok(IpcResponse::Acked)
    );
    match handle_request(&facade, session, IpcRequest::Whoami).await {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.origin, Origin::External("claude-code".into()));
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[tokio::test]
async fn list_processes_returns_the_registered_processes() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "term",
        "sleep 60",
    ));
    match handle_request(&facade, session, IpcRequest::ListProcesses).await {
        Ok(IpcResponse::Processes(processes)) => {
            assert_eq!(processes.len(), 1);
            assert_eq!(processes[0].id, id);
        }
        other => panic!("expected the process list, got {other:?}"),
    }
}

#[tokio::test]
async fn get_process_status_returns_an_in_scope_process() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    match handle_request(
        &facade,
        session,
        IpcRequest::GetProcessStatus { process: id },
    )
    .await
    {
        Ok(IpcResponse::Process(view)) => assert_eq!(view.id, id),
        other => panic!("expected one process, got {other:?}"),
    }
}

#[tokio::test]
async fn get_process_status_reports_unknown_for_a_missing_id() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProcessStatus {
                process: ProcessId::from_raw(999),
            },
        )
        .await,
        Err(IpcError::UnknownProcess)
    );
}

#[tokio::test]
async fn list_projects_is_empty_without_any_loaded() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ListProjects).await,
        Ok(IpcResponse::Projects(Vec::new()))
    );
}

#[tokio::test]
async fn project_status_without_scope_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    // No project loaded, bound, or selected: an unscoped status request is ambiguous.
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProjectStatus { project: None }
        )
        .await,
        Err(IpcError::NoProjectScope)
    );
}

#[tokio::test]
async fn project_status_for_an_unknown_project_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProjectStatus {
                project: Some(ProjectId::from_raw(404)),
            },
        )
        .await,
        Err(IpcError::UnknownProject)
    );
}

#[tokio::test]
async fn binding_an_unknown_process_maps_to_the_wire_error() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::BindSessionProcess {
                process: ProcessId::from_raw(7),
            },
        )
        .await,
        Err(IpcError::UnknownProcess)
    );
}

/// The synthetic peer process group shared by these routing tests — single-sourced from the
/// core test support so it never drifts from the core's own scope tests. Every session opens
/// with it (a real Unix-socket connection always carries a peer group), and [`scoped_terminal`]
/// assigns the same group to the process it binds, so the bind is authentic.
use soloist_core::testing::TEST_PEER_PGID as PEER_PGID;

/// Opens a session as the UDS adapter would for an MCP client running inside a managed process's
/// group — the shape a Soloist-launched agent presents (a real Unix-socket connection always
/// carries a peer group). The routing tests that then bind via [`scoped_terminal`] start here.
fn grouped_session(facade: &Facade) -> SessionId {
    facade.open_session(PeerCredentials::in_group(PEER_PGID))
}

/// Opens a session the transport could not authenticate — no peer group, no directory — which gets
/// the open read tools but can neither bind to a process nor select a project scope.
fn unauthenticated_session(facade: &Facade) -> SessionId {
    facade.open_session(PeerCredentials::unauthenticated())
}

/// Registers a terminal in `project`, gives it the session's peer process group (standing in
/// for the group a real spawn creates), and binds `session` to it — putting that project in
/// scope the way a Soloist-launched agent's session does. The setup every action-routing test
/// shares.
fn scoped_terminal(
    facade: &Facade,
    session: SessionId,
    project: ProjectId,
    name: &str,
) -> ProcessId {
    let id = facade
        .supervisor()
        .register(terminal_registration(project, name, "sleep 60"));
    facade.supervisor().assign_test_group(id, PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(id)
        .expect("an authentic bind to the process the caller runs in");
    id
}

/// Registers and authentically binds an agent for mailbox routing tests.
fn scoped_agent(facade: &Facade, session: SessionId, project: ProjectId, name: &str) -> ProcessId {
    let id = facade
        .supervisor()
        .register(agent_registration(project, name));
    facade.supervisor().assign_test_group(id, PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(id)
        .expect("an authentic bind to the agent the caller runs in");
    id
}

#[tokio::test]
async fn agent_messaging_requests_route_through_the_authenticated_scoped_facade() {
    let facade = facade();
    let session = grouped_session(&facade);
    let agent = scoped_agent(&facade, session, ProjectId::from_raw(1), "lead");

    match handle_request(&facade, session, IpcRequest::AgentRoster).await {
        Ok(IpcResponse::AgentRoster(roster)) => {
            assert_eq!(roster.len(), 1);
            assert_eq!(roster[0].process, agent);
        }
        other => panic!("expected the authenticated roster, got {other:?}"),
    }

    let delivery = match handle_request(
        &facade,
        session,
        IpcRequest::AgentMessageSend {
            recipient: agent,
            body: "Review the adapter".into(),
            todo_id: None,
        },
    )
    .await
    {
        Ok(IpcResponse::AgentMessageDelivery(delivery)) => delivery,
        other => panic!("expected a delivery, got {other:?}"),
    };
    let message_id = delivery.message.id;

    assert!(matches!(
        handle_request(&facade, session, IpcRequest::AgentMessageList).await,
        Ok(IpcResponse::AgentMessages(deliveries)) if deliveries.len() == 1
    ));
    assert!(matches!(
        handle_request(
            &facade,
            session,
            IpcRequest::AgentMessageGet { message_id },
        )
        .await,
        Ok(IpcResponse::AgentMessage(delivery)) if delivery.message.id == message_id
    ));
    assert!(matches!(
        handle_request(
            &facade,
            session,
            IpcRequest::AgentMessageAcknowledge { message_id },
        )
        .await,
        Ok(IpcResponse::AgentMessageDelivery(delivery))
            if delivery.outcome == soloist_core::AgentMessageOutcome::Acknowledged
    ));
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::AgentMessageBroadcast {
                body: "Interfaces are stable".into(),
                todo_id: None,
            },
        )
        .await,
        Ok(IpcResponse::AgentMessageBroadcast(
            soloist_core::AgentBroadcastReceipt {
                deliveries: Vec::new(),
            },
        ))
    );
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::AgentReportCompletion {
                task_message_id: soloist_core::AgentMessageId::from_raw(404),
                todo_id: Some(soloist_core::TodoId::from_raw(404)),
                summary: "Adapter wired".into(),
            },
        )
        .await,
        Err(IpcError::UnknownAgentMessage)
    );
}

#[tokio::test]
async fn starting_an_in_scope_process_is_acked() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StartProcess { process: id }).await,
        Ok(IpcResponse::Acked)
    );
}

#[tokio::test]
async fn stopping_an_idle_in_scope_process_reports_it_was_not_running() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    // Never started, so the stop finds nothing live — the bool the agent reads back.
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StopProcess { process: id }).await,
        Ok(IpcResponse::Stopped(false))
    );
}

#[tokio::test]
async fn renaming_an_in_scope_process_is_acked() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::RenameProcess {
                process: id,
                label: "renamed".into(),
            },
        )
        .await,
        Ok(IpcResponse::Acked)
    );
    assert_eq!(
        facade.process_view(id).expect("registered").label,
        "renamed"
    );
}

#[tokio::test]
async fn closing_an_in_scope_process_removes_it() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    // Never started, so close is a pure removal — acked, and the process leaves the registry.
    assert_eq!(
        handle_request(&facade, session, IpcRequest::CloseProcess { process: id }).await,
        Ok(IpcResponse::Acked)
    );
    assert!(facade.process_view(id).is_none());
}

#[tokio::test]
async fn selecting_a_process_is_acked_and_reported_by_whoami() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::SelectProcess { process: id }).await,
        Ok(IpcResponse::Acked)
    );
    match handle_request(&facade, session, IpcRequest::Whoami).await {
        Ok(IpcResponse::Whoami(who)) => {
            assert_eq!(who.selected_process.map(|p| p.id), Some(id))
        }
        other => panic!("expected a whoami reply, got {other:?}"),
    }
}

#[tokio::test]
async fn sending_input_without_a_wait_returns_no_tail() {
    let facade = facade();
    let mut rx = facade.subscribe();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    facade.supervisor().start(id).expect("terminal starts");
    wait_for(&mut rx, ProcStatus::Running).await;

    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::SendInput {
                process: id,
                input: "ls\r".into(),
                wait_ms: None,
            },
        )
        .await,
        Ok(IpcResponse::InputSent(None))
    );
}

#[tokio::test]
async fn spawning_an_agent_without_scope_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::SpawnAgent {
                tool: "Claude".into(),
                extra_args: Vec::new(),
                prompt: None,
                todo_id: None,
                include_agent_instructions: true,
            },
        )
        .await,
        Err(IpcError::NoProjectScope)
    );
}

/// The command every `spawn_process` routing test below asks to run, and the spec whose variant
/// the trust repo is seeded with — the two must agree, since the gate keys on the variant.
const SPAWNABLE: &str = "sleep 60";

fn spawnable_spec() -> ProcessSpec {
    ProcessSpec {
        command: SPAWNABLE.into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

fn spawn_the_spawnable() -> IpcRequest {
    IpcRequest::SpawnProcess {
        command: SPAWNABLE.into(),
        working_dir: None,
        env: BTreeMap::new(),
        label: None,
    }
}

/// A façade with one project loaded and the trust repo returned, so a test can grant the trust
/// `spawn_process`'s gate looks for. The sole project gives an unbound session its scope.
fn facade_with_a_project(dir: &TempDir) -> (Arc<Facade>, Arc<FakeTrustRepo>, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(dir.path(), Some("p"), None)
        .expect("seed one project")
        .id;
    let trust = Arc::new(FakeTrustRepo::new());
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            trust.clone(),
            projects,
        )
        .build(),
    ));
    (facade, trust, project)
}

#[tokio::test]
async fn spawn_process_routes_into_the_sessions_own_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (facade, trust, project) = facade_with_a_project(&dir);
    trust
        .set_trusted(project, &spawnable_spec().variant_hash())
        .expect("trust the variant in the project");
    let session = grouped_session(&facade);

    match handle_request(&facade, session, spawn_the_spawnable()).await {
        Ok(IpcResponse::Spawned(id)) => {
            let view = facade.process_view(id).expect("the process is registered");
            assert_eq!(
                view.project, project,
                "the spawn lands in the caller's scope"
            );
            assert_eq!(view.kind, ProcessKind::Command);
        }
        other => panic!("expected a Spawned reply, got {other:?}"),
    }
}

#[tokio::test]
async fn spawning_an_untrusted_process_is_refused_as_untrusted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (facade, _trust, _project) = facade_with_a_project(&dir);
    let session = grouped_session(&facade);

    assert_eq!(
        handle_request(&facade, session, spawn_the_spawnable()).await,
        Err(IpcError::Untrusted)
    );
}

#[tokio::test]
async fn spawning_a_process_without_scope_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(&facade, session, spawn_the_spawnable()).await,
        Err(IpcError::NoProjectScope)
    );
}

#[tokio::test]
async fn list_agent_tools_routes_to_the_registry() {
    let facade = facade();
    let session = grouped_session(&facade);
    // The default fakes register no tools; routing is what we assert, not the contents.
    assert!(matches!(
        handle_request(&facade, session, IpcRequest::ListAgentTools).await,
        Ok(IpcResponse::AgentTools(_))
    ));
}

#[tokio::test]
async fn bulk_commands_without_scope_are_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    for request in [
        IpcRequest::StartAllCommands,
        IpcRequest::StopAllCommands,
        IpcRequest::RestartAllCommands,
    ] {
        assert_eq!(
            handle_request(&facade, session, request).await,
            Err(IpcError::NoProjectScope)
        );
    }
}

#[tokio::test]
async fn bulk_start_in_scope_returns_a_summary() {
    let facade = facade();
    let session = grouped_session(&facade);
    // Only a terminal is in scope, so the bulk command start finds nothing to start.
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StartAllCommands).await,
        Ok(IpcResponse::BulkStarted(StartSummary::default()))
    );
}

#[tokio::test]
async fn bulk_stop_in_scope_reports_how_many_were_stopped() {
    let facade = facade();
    let session = grouped_session(&facade);
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StopAllCommands).await,
        Ok(IpcResponse::BulkStopped(0))
    );
}

#[tokio::test]
async fn bulk_restart_in_scope_is_acked() {
    let facade = facade();
    let session = grouped_session(&facade);
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::RestartAllCommands).await,
        Ok(IpcResponse::Acked)
    );
}

#[tokio::test]
async fn output_reads_for_an_unknown_process_are_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    let unknown = ProcessId::from_raw(999);
    for request in [
        IpcRequest::GetProcessOutput {
            process: unknown,
            lines: None,
        },
        IpcRequest::GetProcessRawOutput { process: unknown },
        IpcRequest::SearchOutput {
            process: unknown,
            query: "x".into(),
            limit: None,
        },
        IpcRequest::GetProcessPorts { process: unknown },
        IpcRequest::FlushTerminalPerf { process: unknown },
    ] {
        assert_eq!(
            handle_request(&facade, session, request).await,
            Err(IpcError::UnknownProcess)
        );
    }
}

#[tokio::test]
async fn reading_an_in_scope_processs_output_and_ports() {
    let facade = facade();
    let session = grouped_session(&facade);
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    // Registered but never started: output is empty (not an error), and it has no ports.
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProcessOutput {
                process: id,
                lines: None,
            },
        )
        .await,
        Ok(IpcResponse::Lines(Vec::new()))
    );
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::GetProcessPorts { process: id }
        )
        .await,
        Ok(IpcResponse::Ports(Vec::new()))
    );
    // flush_terminal_perf is a no-op that confirms a known process.
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::FlushTerminalPerf { process: id },
        )
        .await,
        Ok(IpcResponse::Acked)
    );
}

#[tokio::test]
async fn the_read_tools_refuse_an_out_of_scope_process_but_list_stays_cross_project() {
    let facade = facade();
    let session = grouped_session(&facade);
    // The caller's scope is project 1; a process in project 2 is out of scope.
    let here = scoped_terminal(&facade, session, ProjectId::from_raw(1), "here");
    let elsewhere = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(2),
        "elsewhere",
        "sleep 60",
    ));

    // Every per-process read of the foreign process is refused, so its output/status/ports
    // never cross the project boundary — the routing threads the session into the scoped read.
    for request in [
        IpcRequest::GetProcessRawOutput { process: elsewhere },
        IpcRequest::GetProcessOutput {
            process: elsewhere,
            lines: None,
        },
        IpcRequest::GetProcessStatus { process: elsewhere },
        IpcRequest::GetProcessPorts { process: elsewhere },
        IpcRequest::SearchOutput {
            process: elsewhere,
            query: "x".into(),
            limit: None,
        },
        IpcRequest::SearchRawOutput {
            process: elsewhere,
            query: "x".into(),
            limit: None,
        },
        // Even the no-op flush: an ack for a foreign id, where an unknown id refuses, would
        // answer whether that process exists.
        IpcRequest::FlushTerminalPerf { process: elsewhere },
    ] {
        assert_eq!(
            handle_request(&facade, session, request).await,
            Err(IpcError::OutOfScope)
        );
    }

    // `list_processes` still shows both projects' processes (a cross-project overview), so the
    // caller keeps its bearings; the foreign row is identity-only (verified in the core).
    match handle_request(&facade, session, IpcRequest::ListProcesses).await {
        Ok(IpcResponse::Processes(processes)) => {
            let ids: Vec<_> = processes.iter().map(|view| view.id).collect();
            assert!(ids.contains(&here) && ids.contains(&elsewhere));
        }
        other => panic!("expected the process list, got {other:?}"),
    }
}

#[tokio::test]
async fn clear_output_in_scope_is_acked_and_out_of_scope_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    let here = scoped_terminal(&facade, session, ProjectId::from_raw(1), "here");
    let elsewhere = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(2),
        "elsewhere",
        "sleep 60",
    ));
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ClearOutput { process: here }).await,
        Ok(IpcResponse::Acked)
    );
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::ClearOutput { process: elsewhere },
        )
        .await,
        Err(IpcError::OutOfScope)
    );
}

#[tokio::test]
async fn services_list_without_scope_is_refused_and_filters_to_commands_in_scope() {
    let facade = facade();
    let session = grouped_session(&facade);
    // Unscoped: ambiguous, refused.
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ServicesList).await,
        Err(IpcError::NoProjectScope)
    );
    // Scoped to a project whose only process is a terminal: a terminal is not a service, so
    // the list is empty (routing + the command filter, exercised via the app router).
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "shell");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ServicesList).await,
        Ok(IpcResponse::Processes(Vec::new()))
    );
}

#[tokio::test]
async fn wait_for_bound_port_on_a_resting_process_reports_not_running() {
    let facade = facade();
    let session = grouped_session(&facade);
    // Bind to one process in project 1 to put that project in scope, then wait on a *different*,
    // never-started process in the same project: in scope, but with no group it has no port to
    // wait for, so it resolves at once as NotRunning (no wait).
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "bound");
    let resting = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "resting",
        "sleep 60",
    ));
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::WaitForBoundPort {
                process: resting,
                port: 3000,
                timeout_ms: Some(50),
            },
        )
        .await,
        Ok(IpcResponse::PortWait(PortWaitOutcome::NotRunning))
    );
}

#[tokio::test]
async fn wait_for_bound_port_on_an_out_of_scope_process_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    // Scope is project 1; a process in project 2 is out of scope, so the port-bind probe is
    // refused rather than disclosing whether the foreign process bound the port.
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "here");
    let elsewhere = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(2),
        "elsewhere",
        "sleep 60",
    ));
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::WaitForBoundPort {
                process: elsewhere,
                port: 3000,
                timeout_ms: Some(50),
            },
        )
        .await,
        Err(IpcError::OutOfScope)
    );
}

#[tokio::test]
async fn acquiring_a_lease_in_scope_is_granted_then_released() {
    // The lease store must be wired for the round-trip to persist, so this builds its own facade.
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .lock_repo(Arc::new(FakeLockRepo::new()))
        .build(),
    ));
    let session = grouped_session(&facade);
    let owner = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");

    match handle_request(
        &facade,
        session,
        IpcRequest::LockAcquire {
            key: "deploy".into(),
            ttl_ms: Some(30_000),
        },
    )
    .await
    {
        Ok(IpcResponse::LeaseOutcome(AcquireOutcome::Acquired(view))) => {
            assert_eq!(view.owner, owner)
        }
        other => panic!("expected an acquired lease, got {other:?}"),
    }
    match handle_request(
        &facade,
        session,
        IpcRequest::LockStatus {
            key: "deploy".into(),
        },
    )
    .await
    {
        Ok(IpcResponse::LeaseStatus(Some(view))) => assert_eq!(view.owner, owner),
        other => panic!("expected a held lease, got {other:?}"),
    }
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::LockRelease {
                key: "deploy".into(),
            },
        )
        .await,
        Ok(IpcResponse::LeaseReleased(true))
    );
}

#[tokio::test]
async fn a_lease_action_without_scope_is_refused() {
    let facade = facade();
    let session = grouped_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::LockAcquire {
                key: "deploy".into(),
                ttl_ms: Some(30_000),
            },
        )
        .await,
        Err(IpcError::NoProjectScope)
    );
}

#[tokio::test]
async fn an_action_on_another_projects_process_maps_to_out_of_scope() {
    let facade = facade();
    let session = grouped_session(&facade);
    // The session is scoped to project 1; the target lives in project 2.
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "here");
    let elsewhere = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(2),
        "elsewhere",
        "sleep 60",
    ));
    for request in [
        IpcRequest::StartProcess { process: elsewhere },
        IpcRequest::StopProcess { process: elsewhere },
        IpcRequest::RestartProcess { process: elsewhere },
        IpcRequest::RenameProcess {
            process: elsewhere,
            label: "x".into(),
        },
        IpcRequest::CloseProcess { process: elsewhere },
        IpcRequest::SendInput {
            process: elsewhere,
            input: "x".into(),
            wait_ms: None,
        },
    ] {
        assert_eq!(
            handle_request(&facade, session, request).await,
            Err(IpcError::OutOfScope)
        );
    }
}

/// A façade whose settings persist to an in-memory fake, so a toggle round-trips.
fn facade_with_settings() -> Arc<Facade> {
    Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .settings_repo(Arc::new(FakeSettingsRepo::new()))
        .build(),
    ))
}

#[tokio::test]
async fn mcp_tool_groups_returns_the_default_enablement() {
    // A global read — no scope needed, so an unbound session resolves it.
    let facade = facade();
    let session = unauthenticated_session(&facade);
    match handle_request(&facade, session, IpcRequest::McpToolGroups).await {
        Ok(IpcResponse::McpToolGroups(groups)) => {
            assert!(groups.scratchpads && groups.todos && groups.timers);
            assert!(!groups.key_value, "Key-Value defaults off (G10)");
        }
        other => panic!("expected an McpToolGroups reply, got {other:?}"),
    }
}

#[tokio::test]
async fn mcp_tool_groups_reflects_a_persisted_change() {
    let facade = facade_with_settings();
    facade
        .set_mcp_tool_group(McpFeatureGroup::KeyValue, true)
        .expect("enable key-value");
    let session = unauthenticated_session(&facade);
    match handle_request(&facade, session, IpcRequest::McpToolGroups).await {
        Ok(IpcResponse::McpToolGroups(groups)) => assert!(groups.key_value),
        other => panic!("expected an McpToolGroups reply, got {other:?}"),
    }
}

#[tokio::test]
async fn submit_feedback_routes_and_echoes_the_stored_entry() {
    // A global write — no scope needed, so an unbound session resolves it.
    let facade = facade();
    let session = unauthenticated_session(&facade);
    match handle_request(
        &facade,
        session,
        IpcRequest::SubmitFeedback {
            message: "  more keyboard shortcuts please  ".into(),
        },
    )
    .await
    {
        Ok(IpcResponse::Feedback(entry)) => {
            assert_eq!(entry.message, "more keyboard shortcuts please");
        }
        other => panic!("expected a Feedback reply, got {other:?}"),
    }
}

#[tokio::test]
async fn blank_feedback_is_refused_as_a_request_error() {
    let facade = facade();
    let session = unauthenticated_session(&facade);
    match handle_request(
        &facade,
        session,
        IpcRequest::SubmitFeedback {
            message: "  ".into(),
        },
    )
    .await
    {
        Err(err @ IpcError::InvalidFeedback(_)) => assert!(err.is_request_error()),
        other => panic!("expected an InvalidFeedback refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn setup_agent_integration_writes_into_the_scoped_project_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(dir.path(), Some("p"), None)
        .expect("seed one project");
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .build(),
    ));
    // The sole loaded project gives the unbound session its default scope.
    let session = unauthenticated_session(&facade);
    match handle_request(
        &facade,
        session,
        IpcRequest::SetupAgentIntegration {
            file: IntegrationFile::ClaudeMd,
        },
    )
    .await
    {
        Ok(IpcResponse::IntegrationWritten(write)) => {
            assert!(write.created);
            assert_eq!(write.path, dir.path().join("CLAUDE.md"));
        }
        other => panic!("expected an IntegrationWritten reply, got {other:?}"),
    }
}

/// A façade with one project loaded and the template store wired — the sole loaded project
/// gives an unbound session its default scope.
fn facade_with_templates() -> Arc<Facade> {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(
            std::path::Path::new("/tmp/soloist-ipc-template-test"),
            Some("p"),
            None,
        )
        .expect("seed one project");
    Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .template_repo(Arc::new(FakeTemplateRepo::new()))
        .build(),
    ))
}

#[tokio::test]
async fn prompt_templates_route_create_read_update_delete_and_export() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);

    let created = match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateCreate {
            scope: TemplateScope::Project,
            name: "review".into(),
            description: None,
            body: "Review {{diff}}".into(),
        },
    )
    .await
    {
        Ok(IpcResponse::PromptTemplate(view)) => view,
        other => panic!("expected the created template, got {other:?}"),
    };
    assert_eq!(created.placeholders, vec!["diff".to_owned()]);

    match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateUpdate {
            scope: TemplateScope::Project,
            name: "review".into(),
            description: Some("PR review".into()),
            body: "Review {{diff}} for {{focus}}".into(),
            expected_revision: created.revision,
        },
    )
    .await
    {
        Ok(IpcResponse::PromptTemplate(view)) => assert_eq!(view.revision, created.revision + 1),
        other => panic!("expected the updated template, got {other:?}"),
    }

    match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateExport {
            scope: TemplateScope::Project,
            name: "review".into(),
        },
    )
    .await
    {
        Ok(IpcResponse::PromptTemplateExport(exported)) => {
            assert_eq!(exported.body, "Review {{diff}} for {{focus}}");
        }
        other => panic!("expected the export envelope, got {other:?}"),
    }

    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::PromptTemplateDelete {
                scope: TemplateScope::Project,
                name: "review".into(),
            },
        )
        .await,
        Ok(IpcResponse::PromptTemplateDeleted(true))
    );
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::PromptTemplateRead {
                scope: TemplateScope::Project,
                name: "review".into(),
            },
        )
        .await,
        Err(IpcError::UnknownTemplate)
    );
}

/// Seeds the one template the render tests below address.
async fn seed_review_template(facade: &Arc<Facade>, session: SessionId) {
    handle_request(
        facade,
        session,
        IpcRequest::PromptTemplateCreate {
            scope: TemplateScope::Project,
            name: "review".into(),
            description: None,
            body: "Review {{diff}} for {{focus}}".into(),
        },
    )
    .await
    .expect("seed the template");
}

/// A render reaches the core carrying the caller's values: the supplied ones are substituted, the
/// one left out survives as its marker rather than refusing the render, a value the body declares
/// no placeholder for is reported back, and the target the caller chose is echoed.
#[tokio::test]
async fn a_render_substitutes_the_supplied_values_and_reports_the_gaps() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);
    seed_review_template(&facade, session).await;

    match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateRender {
            scope: TemplateScope::Project,
            name: "review".into(),
            values: BTreeMap::from([
                ("diff".to_owned(), "a/b.rs".to_owned()),
                ("dif".to_owned(), "a typo".to_owned()),
            ]),
            policy: MissingPolicy::LeaveVerbatim,
        },
    )
    .await
    {
        Ok(IpcResponse::PromptTemplateRendered(rendered)) => {
            assert_eq!(rendered.text, "Review a/b.rs for {{focus}}");
            assert_eq!(rendered.unfilled, vec!["focus".to_owned()]);
            assert_eq!(rendered.unknown, vec!["dif".to_owned()]);
        }
        other => panic!("expected a rendered prompt, got {other:?}"),
    }
}

/// The strict policy travels the wire and is honoured by the core: a caller whose protocol cannot
/// carry a warning gets the render refused, naming every value still to supply so one retry can
/// fill them all. The refusal is its own wire error — folded into a generic internal failure, an
/// adapter could not tell a fixable mistake from a genuine fault.
#[tokio::test]
async fn a_strict_render_is_refused_on_the_wire_naming_every_missing_value() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);
    seed_review_template(&facade, session).await;

    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::PromptTemplateRender {
                scope: TemplateScope::Project,
                name: "review".into(),
                values: BTreeMap::new(),
                policy: MissingPolicy::Strict,
            },
        )
        .await,
        Err(IpcError::MissingTemplateValues {
            names: vec!["diff".to_owned(), "focus".to_owned()],
        })
    );
}

/// A strict render that lacks nothing is not refused — the policy only bites on a gap.
#[tokio::test]
async fn a_strict_render_with_every_value_supplied_succeeds() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);
    seed_review_template(&facade, session).await;

    match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateRender {
            scope: TemplateScope::Project,
            name: "review".into(),
            values: BTreeMap::from([
                ("diff".to_owned(), "a/b.rs".to_owned()),
                ("focus".to_owned(), "leaks".to_owned()),
            ]),
            policy: MissingPolicy::Strict,
        },
    )
    .await
    {
        Ok(IpcResponse::PromptTemplateRendered(rendered)) => {
            assert_eq!(rendered.text, "Review a/b.rs for leaks");
        }
        other => panic!("expected a rendered prompt, got {other:?}"),
    }
}

#[tokio::test]
async fn rendering_a_template_that_does_not_exist_is_refused_on_the_wire() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::PromptTemplateRender {
                scope: TemplateScope::Project,
                name: "absent".into(),
                values: BTreeMap::new(),
                policy: MissingPolicy::LeaveVerbatim,
            },
        )
        .await,
        Err(IpcError::UnknownTemplate)
    );
}

#[tokio::test]
async fn an_unscoped_template_list_merges_global_and_project_rows() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);
    for (scope, name) in [
        (TemplateScope::Global, "shared"),
        (TemplateScope::Project, "mine"),
    ] {
        handle_request(
            &facade,
            session,
            IpcRequest::PromptTemplateCreate {
                scope,
                name: name.into(),
                description: None,
                body: "body".into(),
            },
        )
        .await
        .expect("create");
    }

    match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateList { scope: None },
    )
    .await
    {
        Ok(IpcResponse::PromptTemplates(rows)) => {
            assert_eq!(rows.len(), 2, "global and project rows merge");
        }
        other => panic!("expected the merged list, got {other:?}"),
    }
}

#[tokio::test]
async fn a_stale_template_update_maps_to_the_wire_conflict() {
    let facade = facade_with_templates();
    let session = unauthenticated_session(&facade);
    handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateCreate {
            scope: TemplateScope::Project,
            name: "review".into(),
            description: None,
            body: "one".into(),
        },
    )
    .await
    .expect("create");

    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::PromptTemplateUpdate {
                scope: TemplateScope::Project,
                name: "review".into(),
                description: None,
                body: "two".into(),
                expected_revision: 9,
            },
        )
        .await,
        Err(IpcError::TemplateRevisionConflict {
            expected: Some(9),
            actual: Some(1),
        })
    );
}

#[tokio::test]
async fn setup_agent_integration_with_no_scope_is_refused() {
    let facade = facade();
    let session = unauthenticated_session(&facade);
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::SetupAgentIntegration {
                file: IntegrationFile::AgentsMd,
            },
        )
        .await,
        Err(IpcError::NoProjectScope)
    );
}

/// A façade over a repository fake and a reachable forge with one project open, plus a session
/// sitting in that project's directory — the routing test's alternate composition root for version
/// control. A forge nothing asks anything of costs nothing, so it is always there.
fn git_facade(repository: FakeGitRepository) -> (Arc<Facade>, Arc<FakeTrustRepo>, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().canonicalize().expect("canonical root");
    let projects = Arc::new(FakeProjectRepo::new());
    projects.upsert(&root, None, None).expect("add project");
    let trust = Arc::new(FakeTrustRepo::new());
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            trust.clone(),
            projects,
        )
        .git_repository(Arc::new(repository))
        .git_forge(Arc::new(FakeGitForge::ready()))
        .build(),
    ));
    (facade, trust, dir)
}

/// Every version-control request routes to its own scoped behaviour. The dispatch is one flat arm
/// per request, so the compiler insists each is routed — but not that it is routed to the right
/// thing, which is what a copy-pasted arm gets wrong and what this catches.
#[tokio::test]
async fn each_version_control_request_routes_to_its_own_behaviour() {
    let mut status = soloist_core::testing::git_status("main");
    // Staged, so the commit has something to record and reaches the port rather than the core's
    // nothing-staged guard.
    status.changes.push(soloist_core::testing::file_change(
        "src/main.rs",
        Some(soloist_core::ChangeKind::Modified),
        None,
    ));
    let repository =
        FakeGitRepository::reporting(status).branching(soloist_core::testing::branches(Vec::new()));
    let (facade, trust, dir) = git_facade(repository.clone());
    let project = facade
        .projects_snapshot()
        .expect("projects")
        .first()
        .expect("one project")
        .id;
    trust.set_project_trusted(project).expect("trust");
    let session = facade.open_session(PeerCredentials::in_dir(
        dir.path().canonicalize().expect("canonical"),
    ));

    assert!(matches!(
        handle_request(&facade, session, IpcRequest::GitStatus).await,
        Ok(IpcResponse::GitStatus(_)),
    ));
    assert!(matches!(
        handle_request(&facade, session, IpcRequest::GitBranches).await,
        Ok(IpcResponse::GitBranches(_)),
    ));
    let hunk = soloist_core::HunkRange {
        old_start: 3,
        old_lines: 2,
        new_start: 3,
        new_lines: 4,
    };
    for request in [
        IpcRequest::GitStage {
            path: "src/main.rs".into(),
            hunk: None,
        },
        // A hunk named on the wire must survive the routing: dropping it here would quietly widen
        // acting on part of a change into acting on the whole file.
        IpcRequest::GitStage {
            path: "src/main.rs".into(),
            hunk: Some(hunk),
        },
        IpcRequest::GitCommit {
            message: "a subject".into(),
            amend: false,
        },
        IpcRequest::GitPush { progress: false },
        IpcRequest::GitPull { progress: false },
        IpcRequest::GitFetch { progress: false },
        IpcRequest::GitCreateBranch {
            name: "topic".into(),
        },
        IpcRequest::GitStash,
        IpcRequest::GitPopStash,
    ] {
        let routed = handle_request(&facade, session, request.clone()).await;
        assert!(
            matches!(routed, Ok(IpcResponse::Acked)),
            "{request:?} did not route: {routed:?}"
        );
    }

    // What each one did, in the order it was asked for — a mis-routed arm reads back as the wrong
    // change, or as the same change twice.
    assert_eq!(
        repository.changes(),
        vec![
            GitChange::Stage {
                path: "src/main.rs".into(),
                original_path: None,
            },
            GitChange::StageHunk {
                path: "src/main.rs".into(),
                hunk,
            },
            GitChange::Commit {
                message: "a subject".into(),
                amend: false,
            },
            GitChange::Sync {
                op: SyncOp::Publish,
                prompting: Prompting::Denied,
            },
            GitChange::Sync {
                op: SyncOp::Pull,
                prompting: Prompting::Denied,
            },
            GitChange::Sync {
                op: SyncOp::Fetch,
                prompting: Prompting::Denied,
            },
            GitChange::Branch {
                op: BranchOp::Create,
                name: "topic".into(),
            },
            GitChange::Stash { op: StashOp::Save },
            GitChange::Stash { op: StashOp::Pop },
        ],
    );
}

/// A version-control refusal crosses the wire as its own word, so the surface on the other side can
/// tell one refusal from another without reading the sentence.
#[tokio::test]
async fn a_refused_change_crosses_the_wire_as_the_word_that_classifies_it() {
    let (facade, _trust, dir) = git_facade(FakeGitRepository::reporting(
        soloist_core::testing::git_status("main"),
    ));
    let session = facade.open_session(PeerCredentials::in_dir(
        dir.path().canonicalize().expect("canonical"),
    ));

    // The project has not been trusted, which is the refusal every change shares.
    let refused = handle_request(
        &facade,
        session,
        IpcRequest::GitCommit {
            message: "a subject".into(),
            amend: false,
        },
    )
    .await;

    assert!(matches!(
        refused,
        Err(IpcError::Git {
            reason: GitRefusal::ProjectUntrusted,
            ..
        }),
    ));
}

/// Serves `request` with a receiver kept, answering what it did and everything it said about itself
/// on the way. Bounded by the request itself: the reports channel closes when the request ends, so
/// the drain cannot outlive it.
async fn served_reporting(
    facade: &Arc<Facade>,
    session: SessionId,
    request: IpcRequest,
) -> (IpcResult, Vec<String>) {
    let (reports, mut reported) = mpsc::channel(8);
    let answer = super::handle_request(facade, session, request, reports).await;
    let mut said = Vec::new();
    while let Some(note) = reported.recv().await {
        said.push(note);
    }
    (answer, said)
}

#[tokio::test]
async fn a_push_that_asked_to_be_told_hears_the_exchange_and_one_that_did_not_hears_nothing() {
    let status = soloist_core::testing::git_status("main");
    let repository =
        FakeGitRepository::reporting(status).branching(soloist_core::testing::branches(Vec::new()));
    let (facade, trust, dir) = git_facade(repository.clone());
    let project = facade
        .projects_snapshot()
        .expect("projects")
        .first()
        .expect("one project")
        .id;
    trust.set_project_trusted(project).expect("trust");
    let session = facade.open_session(PeerCredentials::in_dir(
        dir.path().canonicalize().expect("canonical"),
    ));

    let (asked, heard) =
        served_reporting(&facade, session, IpcRequest::GitPush { progress: true }).await;
    let (unasked, silence) =
        served_reporting(&facade, session, IpcRequest::GitPush { progress: false }).await;

    assert!(matches!(asked, Ok(IpcResponse::Acked)));
    assert!(matches!(unasked, Ok(IpcResponse::Acked)));
    assert_eq!(
        heard,
        vec![soloist_core::testing::REMARK.to_string()],
        "a caller that asked to be told heard nothing",
    );
    assert!(
        silence.is_empty(),
        "a caller that never asked was told anyway: {silence:?}",
    );
}

/// A proposal publishes the branch when the remote does not hold it, so it carries the same slow
/// exchange a push does — and the same choice about hearing it.
#[tokio::test]
async fn a_proposal_that_asked_to_be_told_hears_the_branch_being_published_and_one_that_did_not_hears_nothing(
) {
    let status = soloist_core::testing::git_status("main");
    let (facade, trust, dir) = git_facade(FakeGitRepository::reporting(status));
    let project = facade
        .projects_snapshot()
        .expect("projects")
        .first()
        .expect("one project")
        .id;
    trust.set_project_trusted(project).expect("trust");
    let session = facade.open_session(PeerCredentials::in_dir(
        dir.path().canonicalize().expect("canonical"),
    ));
    let proposing = |progress| IpcRequest::GitCreatePullRequest {
        new: NewPullRequest {
            title: "Add the thing".into(),
            body: "It does the thing.".into(),
            base: "main".into(),
            draft: false,
        },
        progress,
    };

    let (asked, heard) = served_reporting(&facade, session, proposing(true)).await;
    let (unasked, silence) = served_reporting(&facade, session, proposing(false)).await;

    assert!(matches!(asked, Ok(IpcResponse::GitPullRequestCreated(_))));
    assert!(matches!(unasked, Ok(IpcResponse::GitPullRequestCreated(_))));
    assert_eq!(
        heard,
        vec![soloist_core::testing::REMARK.to_string()],
        "a caller that asked to be told heard nothing while its branch was published",
    );
    assert!(
        silence.is_empty(),
        "a caller that never asked was told anyway: {silence:?}",
    );
}

fn request_the_spawnables_trust() -> IpcRequest {
    IpcRequest::RequestCommandTrust {
        command: SPAWNABLE.into(),
        working_dir: None,
        env: BTreeMap::new(),
        label: None,
        reason: "the release build needs it".into(),
    }
}

#[tokio::test]
async fn requesting_trust_records_it_in_the_sessions_own_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (facade, _trust, project) = facade_with_a_project(&dir);
    let session = grouped_session(&facade);
    let caller = scoped_terminal(&facade, session, project, "shell");

    let opened = match handle_request(&facade, session, request_the_spawnables_trust()).await {
        Ok(IpcResponse::TrustRequestOpened(outcome)) => outcome,
        other => panic!("expected a recorded trust request, got {other:?}"),
    };

    assert_eq!(opened.state, TrustRequestState::Pending);
    let id = opened
        .request_id
        .expect("a pending request has an id to poll");
    let open = facade.pending_trust_requests(project);
    assert_eq!(open.len(), 1);
    assert_eq!(
        (open[0].project, open[0].requested_by),
        (project, caller),
        "scope and attribution come from the session, not the request"
    );
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::TrustRequestStatus { request: id }
        )
        .await,
        Ok(IpcResponse::TrustRequest(TrustRequestState::Pending))
    );
}

#[tokio::test]
async fn requesting_trust_for_an_already_trusted_variant_asks_nobody() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (facade, trust, project) = facade_with_a_project(&dir);
    trust
        .set_trusted(project, &spawnable_spec().variant_hash())
        .expect("trust the variant in the project");
    let session = grouped_session(&facade);
    scoped_terminal(&facade, session, project, "shell");

    assert_eq!(
        handle_request(&facade, session, request_the_spawnables_trust()).await,
        Ok(IpcResponse::TrustRequestOpened(TrustRequestOutcome {
            request_id: None,
            state: TrustRequestState::Granted,
        }))
    );
    assert!(facade.pending_trust_requests(project).is_empty());
}

/// Deciding a trust request is absent from the wire protocol altogether, which is the boundary the
/// whole design rests on: a session-scoped caller must not be able to approve its own request, and
/// the way that is guaranteed is that there is no request it could send to try.
#[test]
fn deciding_a_trust_request_is_not_reachable_over_ipc() {
    for op in [
        "approve_trust_request",
        "deny_trust_request",
        "trust_request_approve",
        "trust_request_deny",
        "revoke_command_trust",
    ] {
        let attempt = serde_json::json!({ "op": op, "request": 1, "variant_hash": "abc" });
        assert!(
            serde_json::from_value::<IpcRequest>(attempt).is_err(),
            "`{op}` must not exist on the IPC surface: approving is the local user's authority"
        );
    }
}
