use super::*;
use soloist_core::testing::{
    terminal_registration, FakeLockRepo, FakeProjectRepo, FakePromptTemplateRepo, FakeSettingsRepo,
    FakeSpawner, FakeTrustRepo,
};
use soloist_core::{
    AcquireOutcome, CorePorts, DomainEvent, IntegrationFile, McpFeatureGroup, Origin, ProcStatus,
    ProcessId, ProjectRepo, PromptScope, StartSummary, TokioClock,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// A façade over in-memory fakes — an alternate composition root, the same way the core's
/// own tests build one. Routing is what we exercise here; the behaviour behind each call
/// is tested in the core. Returned as an [`Arc`] because [`handle_request`] takes the façade by
/// shared handle (it clones it onto the blocking pool for the synchronous dispatch).
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
    assert_eq!(
        handle_request(&facade, session, IpcRequest::ListProjects).await,
        Ok(IpcResponse::Projects(Vec::new()))
    );
}

#[tokio::test]
async fn project_status_without_scope_is_refused() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
        .bind_session_process(session, id)
        .expect("an authentic bind to the process the caller runs in");
    id
}

#[tokio::test]
async fn starting_an_in_scope_process_is_acked() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
    let id = scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StartProcess { process: id }).await,
        Ok(IpcResponse::Acked)
    );
}

#[tokio::test]
async fn stopping_an_idle_in_scope_process_reports_it_was_not_running() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
    assert_eq!(
        handle_request(
            &facade,
            session,
            IpcRequest::SpawnAgent {
                tool: "Claude".into(),
                extra_args: Vec::new(),
            },
        )
        .await,
        Err(IpcError::NoProjectScope)
    );
}

#[tokio::test]
async fn list_agent_tools_routes_to_the_registry() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
    // The default fakes register no tools; routing is what we assert, not the contents.
    assert!(matches!(
        handle_request(&facade, session, IpcRequest::ListAgentTools).await,
        Ok(IpcResponse::AgentTools(_))
    ));
}

#[tokio::test]
async fn bulk_commands_without_scope_are_refused() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::StopAllCommands).await,
        Ok(IpcResponse::BulkStopped(0))
    );
}

#[tokio::test]
async fn bulk_restart_in_scope_is_acked() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
    scoped_terminal(&facade, session, ProjectId::from_raw(1), "term");
    assert_eq!(
        handle_request(&facade, session, IpcRequest::RestartAllCommands).await,
        Ok(IpcResponse::Acked)
    );
}

#[tokio::test]
async fn output_reads_for_an_unknown_process_are_refused() {
    let facade = facade();
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(Some(PEER_PGID));
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
    let session = facade.open_session(None);
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
    let session = facade.open_session(None);
    match handle_request(&facade, session, IpcRequest::McpToolGroups).await {
        Ok(IpcResponse::McpToolGroups(groups)) => assert!(groups.key_value),
        other => panic!("expected an McpToolGroups reply, got {other:?}"),
    }
}

#[tokio::test]
async fn submit_feedback_routes_and_echoes_the_stored_entry() {
    // A global write — no scope needed, so an unbound session resolves it.
    let facade = facade();
    let session = facade.open_session(None);
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
    let session = facade.open_session(None);
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
    let session = facade.open_session(None);
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
        .prompt_template_repo(Arc::new(FakePromptTemplateRepo::new()))
        .build(),
    ))
}

#[tokio::test]
async fn prompt_templates_route_create_read_update_delete_and_export() {
    let facade = facade_with_templates();
    let session = facade.open_session(None);

    let created = match handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateCreate {
            scope: PromptScope::Project,
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
            scope: PromptScope::Project,
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
            scope: PromptScope::Project,
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
                scope: PromptScope::Project,
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
                scope: PromptScope::Project,
                name: "review".into(),
            },
        )
        .await,
        Err(IpcError::UnknownPromptTemplate)
    );
}

#[tokio::test]
async fn an_unscoped_template_list_merges_global_and_project_rows() {
    let facade = facade_with_templates();
    let session = facade.open_session(None);
    for (scope, name) in [
        (PromptScope::Global, "shared"),
        (PromptScope::Project, "mine"),
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
    let session = facade.open_session(None);
    handle_request(
        &facade,
        session,
        IpcRequest::PromptTemplateCreate {
            scope: PromptScope::Project,
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
                scope: PromptScope::Project,
                name: "review".into(),
                description: None,
                body: "two".into(),
                expected_revision: 9,
            },
        )
        .await,
        Err(IpcError::PromptTemplateRevisionConflict {
            expected: Some(9),
            actual: Some(1),
        })
    );
}

#[tokio::test]
async fn setup_agent_integration_with_no_scope_is_refused() {
    let facade = facade();
    let session = facade.open_session(None);
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
