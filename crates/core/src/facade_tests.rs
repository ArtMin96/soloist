use super::*;
use crate::ids::ProjectId;
use crate::ports::{TokioClock, TrustRepo};
use crate::process::ProcStatus;
use crate::supervisor::{Registration, SupervisorError};
use crate::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

fn facade(spawner: FakeSpawner) -> (Facade, Arc<FakeTrustRepo>) {
    let trust = Arc::new(FakeTrustRepo::new());
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(spawner),
            Arc::new(TokioClock),
            trust.clone(),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    );
    (facade, trust)
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
async fn the_facade_registers_starts_and_stops_a_process() {
    let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
    let mut rx = facade.subscribe();

    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "term",
        "sleep 60",
    ));
    // Starting an ungated terminal cannot fail the trust gate.
    facade
        .supervisor()
        .start(id)
        .expect("ungated terminal starts");
    assert_eq!(facade.snapshot().len(), 1);
    wait_for(&mut rx, ProcStatus::Running).await;

    // Stop routes through the same supervisor the snapshot reflects.
    assert!(facade.supervisor().stop(id));
    wait_for(&mut rx, ProcStatus::Stopped).await;
}

#[tokio::test]
async fn the_trust_gate_is_enforced_through_the_facade() {
    let (facade, trust) = facade(FakeSpawner::exits_on_terminate());
    let config =
        crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
    let spec = config.processes.get("Web").cloned().expect("Web");
    let project = ProjectId::from_raw(1);
    let id = facade.supervisor().register(Registration::command(
        project,
        Path::new("/p"),
        "Web",
        &spec,
    ));

    assert!(matches!(
        facade.supervisor().start(id),
        Err(SupervisorError::Untrusted)
    ));

    trust
        .set_trusted(project, &spec.variant_hash())
        .expect("trust");
    facade.supervisor().start(id).expect("start once trusted");
}

#[tokio::test]
async fn the_facade_exposes_the_agent_registry_and_detection() {
    use crate::agents::AgentTool;
    use crate::testing::{FakeAgentToolRepo, FakeVersionProbe};

    let tools = AgentTool::builtin_defaults();
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(tools.clone())))
        .version_probe(Arc::new(FakeVersionProbe::new(&["claude"])))
        .build(),
    );

    // The registry and auto-detection both route through the one agents context.
    assert_eq!(facade.agents().list_tools().expect("list"), tools);
    let detected = facade.agents().detect_installed().await.expect("detect");
    let claude = detected
        .iter()
        .find(|d| d.tool.command == "claude")
        .expect("claude detected");
    assert!(claude.installed, "the probed CLI is reported installed");
}

#[tokio::test]
async fn trust_command_makes_an_untrusted_command_startable() {
    let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        crate::config::config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
    )
    .expect("write solo.yml");
    let project = facade.load_project(dir.path()).expect("load");

    // Registered untrusted: the read model flags it and the gate refuses to start it.
    let web = || {
        facade
            .snapshot()
            .into_iter()
            .find(|p| p.label == "Web")
            .expect("Web")
    };
    assert!(web().requires_trust);
    assert!(matches!(
        facade.supervisor().start(web().id),
        Err(SupervisorError::Untrusted)
    ));

    facade
        .trust_command(project.id, "Web")
        .expect("trust the command");

    // The flag clears and the same start path now succeeds.
    assert!(!web().requires_trust);
    facade
        .supervisor()
        .start(web().id)
        .expect("starts once trusted");
}

#[tokio::test]
async fn trust_command_rejects_an_unknown_command() {
    let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        crate::config::config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n",
    )
    .expect("write solo.yml");
    let project = facade.load_project(dir.path()).expect("load");

    assert!(matches!(
        facade.trust_command(project.id, "Missing"),
        Err(TrustCommandError::NotFound)
    ));
}

/// A façade seeded with the built-in agent tools (so `"Claude"` resolves) over an
/// in-memory project repo, for the agent-launch path.
fn facade_with_tools(spawner: FakeSpawner) -> Facade {
    use crate::agents::AgentTool;
    use crate::testing::FakeAgentToolRepo;
    Facade::new(
        CorePorts::builder(
            Arc::new(spawner),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(
            AgentTool::builtin_defaults(),
        )))
        .build(),
    )
}

#[tokio::test]
async fn launch_agent_registers_and_starts_an_agent_in_the_project() {
    use crate::process::ProcessKind;

    let facade = facade_with_tools(FakeSpawner::exits_on_terminate());
    let mut rx = facade.subscribe();
    let dir = tempfile::tempdir().expect("temp dir");
    let project = facade.load_project(dir.path()).expect("load");

    let id = facade
        .launch_agent(project.id, "Claude", Vec::new())
        .expect("launch");

    // It appears as an ungated Agent-kind process labelled by the tool, and starts.
    let view = facade
        .snapshot()
        .into_iter()
        .find(|p| p.id == id)
        .expect("launched agent in snapshot");
    assert_eq!(view.kind, ProcessKind::Agent);
    assert_eq!(view.label, "Claude");
    assert!(
        !view.requires_trust,
        "a launched agent is never trust-gated"
    );
    wait_for(&mut rx, ProcStatus::Running).await;
}

#[tokio::test]
async fn launch_agent_rejects_an_unknown_tool() {
    let facade = facade_with_tools(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    let project = facade.load_project(dir.path()).expect("load");

    assert!(matches!(
        facade.launch_agent(project.id, "Nonexistent", Vec::new()),
        Err(LaunchAgentError::UnknownTool)
    ));
}

#[tokio::test]
async fn launch_agent_rejects_an_unknown_project() {
    let facade = facade_with_tools(FakeSpawner::exits_on_terminate());

    assert!(matches!(
        facade.launch_agent(ProjectId::from_raw(9999), "Claude", Vec::new()),
        Err(LaunchAgentError::UnknownProject)
    ));
}
