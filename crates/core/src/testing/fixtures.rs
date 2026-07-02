//! Shared registration and façade fixtures for driving the supervisor and spawn paths end
//! to end, reused by the core's own tests and, via the `testing` feature, by adapter-crate
//! integration tests.

use std::collections::BTreeMap;

use crate::ids::ProjectId;
use crate::ports::{PtySize, SpawnSpec};
use crate::process::ProcessKind;
use crate::supervisor::Registration;

/// Builds a [`Registration`] for an ungated terminal running `command` — the minimal
/// launched-process fixture for exercising the supervisor thread (register → start →
/// stop) end to end across crates. A terminal is ungated, so the trust gate is never
/// consulted; the working directory is `.` (irrelevant to a self-contained command).
pub fn terminal_registration(project: ProjectId, name: &str, command: &str) -> Registration {
    Registration::launched(
        project,
        ProcessKind::Terminal,
        name,
        SpawnSpec {
            command: command.into(),
            working_dir: ".".into(),
            env: BTreeMap::new(),
            size: PtySize::default(),
        },
    )
}

/// Builds a [`Registration`] for an agent named `name` — a lineage-tree node that can be
/// registered without starting, for lineage and identity tests. The command is irrelevant:
/// these fixtures are never spawned for real (the [`FakeSpawner`] stands in when they are).
pub fn agent_registration(project: ProjectId, name: &str) -> Registration {
    Registration::launched(
        project,
        ProcessKind::Agent,
        name,
        SpawnSpec {
            command: "agent".into(),
            working_dir: ".".into(),
            env: BTreeMap::new(),
            size: PtySize::default(),
        },
    )
}

/// A façade with one project loaded and one launchable agent tool named `"worker"`, so
/// `spawn_agent` runs end to end against fakes. Returns the façade and the loaded project's
/// id (the sole project, so an unbound session still resolves its scope to it). Compiled for
/// the core's own tests only — it asserts via `expect`, which the core denies outside test
/// builds (the same rule as the event waiters).
#[cfg(test)]
pub fn facade_with_agent_tool() -> (crate::facade::Facade, ProjectId) {
    use std::path::Path;
    use std::sync::Arc;

    use crate::agents::{AgentKind, AgentTool, PromptMode};
    use crate::facade::Facade;
    use crate::ports::{CorePorts, ProjectRepo};
    use crate::testing::{
        FakeAgentToolRepo, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock,
    };

    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(Path::new("/"), Some("proj"), None)
        .expect("seed a project")
        .id;
    let tool = AgentTool {
        name: "worker".into(),
        command: "true".into(),
        default_args: Vec::new(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::AppendedArg,
    };
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(MockClock::new()),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(vec![tool])))
        .build(),
    );
    (facade, project)
}
