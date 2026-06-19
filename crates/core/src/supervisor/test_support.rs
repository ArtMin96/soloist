//! Shared scaffolding for the supervisor's unit tests: a [`Harness`] that wires a
//! [`Supervisor`] over fakes and a mock clock, plus the registration and event-stream
//! helpers its tests reuse. Lives in one place so every submodule's `#[cfg(test)] mod
//! tests` builds against the same fixtures rather than re-rolling them.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind};
use crate::testing::{
    FakeOrphanControl, FakeRuntimeState, FakeSpawner, FakeTrustRepo, MockClock,
    RecordingLockReleaser,
};

use super::{Registration, Supervisor};

pub(crate) const PROJECT: ProjectId = ProjectId::from_raw(1);

pub(crate) struct Harness {
    pub(crate) sup: Supervisor,
    pub(crate) trust: Arc<FakeTrustRepo>,
    pub(crate) locks: RecordingLockReleaser,
    pub(crate) clock: MockClock,
    pub(crate) runtime: Arc<FakeRuntimeState>,
    pub(crate) orphans: Arc<FakeOrphanControl>,
    pub(crate) rx: broadcast::Receiver<DomainEvent>,
}

pub(crate) fn harness(spawner: FakeSpawner) -> Harness {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let trust = Arc::new(FakeTrustRepo::new());
    let locks = RecordingLockReleaser::new();
    let clock = MockClock::new();
    let runtime = Arc::new(FakeRuntimeState::new());
    let orphans = Arc::new(FakeOrphanControl::new());
    let sup = Supervisor::new(
        Arc::new(spawner),
        Arc::new(clock.clone()),
        trust.clone(),
        Arc::new(locks.clone()),
        runtime.clone(),
        orphans.clone(),
        bus,
    );
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

pub(crate) fn terminal(sup: &Supervisor, command: &str) -> ProcessId {
    sup.register(Registration::launched(
        PROJECT,
        ProcessKind::Terminal,
        "shell",
        spawn_spec(command),
    ))
}

pub(crate) async fn next_to(rx: &mut broadcast::Receiver<DomainEvent>) -> ProcStatus {
    next_change(rx).await.0
}

pub(crate) async fn next_change(
    rx: &mut broadcast::Receiver<DomainEvent>,
) -> (ProcStatus, Option<i32>) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { to, exit_code, .. }) => return (to, exit_code),
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

pub(crate) async fn wait_all(
    rx: &mut broadcast::Receiver<DomainEvent>,
    ids: &[ProcessId],
    target: ProcStatus,
) {
    let mut remaining: HashSet<ProcessId> = ids.iter().copied().collect();
    while !remaining.is_empty() {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { id, to, .. }) if to == target => {
                remaining.remove(&id);
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

pub(crate) fn status_of(sup: &Supervisor, id: ProcessId) -> ProcStatus {
    sup.snapshot()
        .into_iter()
        .find(|view| view.id == id)
        .map(|view| view.status)
        .expect("process is registered")
}
