//! Shared scaffolding for the supervisor's unit tests: a [`Harness`] that wires a
//! [`Supervisor`] over fakes and a mock clock, plus the registration and event-stream
//! helpers its tests reuse. Lives in one place so every submodule's `#[cfg(test)] mod
//! tests` builds against the same fixtures rather than re-rolling them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{CorePorts, PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind};
use crate::shellenv::{NoopShellEnvProbe, ShellEnvProbe};
use crate::testing::{
    FakeOrphanControl, FakeProjectRepo, FakeRuntimeState, FakeSpawner, FakeTrustRepo, MockClock,
    RecordingLockReleaser,
};

use super::{Registration, Supervisor};

// The event-stream waiters live once in `crate::testing`; the supervisor's submodule tests
// reach them through this scaffolding alongside the supervisor-specific fixtures.
pub(crate) use crate::testing::{next_change, next_to, wait_all};

pub(crate) const PROJECT: ProjectId = ProjectId::from_raw(1);

pub(crate) struct Harness {
    pub(crate) sup: Arc<Supervisor>,
    pub(crate) trust: Arc<FakeTrustRepo>,
    pub(crate) locks: RecordingLockReleaser,
    pub(crate) clock: MockClock,
    pub(crate) runtime: Arc<FakeRuntimeState>,
    pub(crate) orphans: Arc<FakeOrphanControl>,
    pub(crate) rx: broadcast::Receiver<DomainEvent>,
}

pub(crate) fn harness(spawner: FakeSpawner) -> Harness {
    harness_with_shell_env(spawner, Arc::new(NoopShellEnvProbe), BTreeMap::new())
}

/// A [`Harness`] whose supervisor resolves spawn environments through `shell_env_probe`
/// over `app_env` — for the tests that prove the captured shell environment reaches a
/// spawn. The default [`harness`] passes [`NoopShellEnvProbe`] and an empty app env, so a
/// process's environment is exactly its own overrides (the pre-capture behavior).
pub(crate) fn harness_with_shell_env(
    spawner: FakeSpawner,
    shell_env_probe: Arc<dyn ShellEnvProbe>,
    app_env: BTreeMap<String, String>,
) -> Harness {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let trust = Arc::new(FakeTrustRepo::new());
    let locks = RecordingLockReleaser::new();
    let clock = MockClock::new();
    let runtime = Arc::new(FakeRuntimeState::new());
    let orphans = Arc::new(FakeOrphanControl::new());
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(clock.clone()),
        trust.clone(),
        Arc::new(FakeProjectRepo::new()),
    )
    .locks(Arc::new(locks.clone()))
    .runtime(runtime.clone())
    .orphan_control(orphans.clone())
    .shell_env_probe(shell_env_probe)
    .app_env(app_env)
    .build();
    let sup = Arc::new(Supervisor::new(&ports, bus));
    Harness {
        sup,
        trust,
        locks,
        clock,
        runtime,
        orphans,
        rx,
    }
}

pub(crate) fn spawn_spec(command: &str) -> SpawnSpec {
    SpawnSpec {
        command: command.into(),
        working_dir: PathBuf::from("/"),
        env: BTreeMap::new(),
        size: PtySize::default(),
    }
}

pub(crate) fn command_spec(command: &str, auto_start: bool) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

/// A trust-gated command that auto-restarts after a crash but does not auto-start — the
/// fixture the restart-policy tests register.
pub(crate) fn auto_restart_spec(command: &str) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: false,
        auto_restart: true,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

pub(crate) fn terminal(sup: &Supervisor, command: &str) -> ProcessId {
    sup.register(Registration::launched(
        PROJECT,
        ProcessKind::Terminal,
        "shell",
        spawn_spec(command),
    ))
}

pub(crate) fn status_of(sup: &Supervisor, id: ProcessId) -> ProcStatus {
    sup.snapshot()
        .into_iter()
        .find(|view| view.id == id)
        .map(|view| view.status)
        .expect("process is registered")
}
