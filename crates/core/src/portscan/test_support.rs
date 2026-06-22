//! Shared scaffolding for the portscan domain's tests (the scanner and the readiness
//! waiter): a running supervisor over fakes plus the shared bus and mock clock, so both
//! test files build against the same fixtures instead of re-rolling them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{CorePorts, PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind, ProcessView};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{wait_all, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

pub(crate) const PROJECT: ProjectId = ProjectId::from_raw(1);

/// A clock step generous enough to fire whichever single timer is pending each round (a
/// scan/poll interval or a readiness timeout), so tests advance deterministically without
/// knowing the exact interval.
pub(crate) const ADVANCE_STEP: Duration = Duration::from_secs(10);

pub(crate) struct Setup {
    pub(crate) sup: Arc<Supervisor>,
    pub(crate) clock: MockClock,
    pub(crate) bus: EventBus,
    pub(crate) rx: broadcast::Receiver<DomainEvent>,
}

pub(crate) fn setup() -> Setup {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let clock = MockClock::new();
    let ports = CorePorts::builder(
        // Stays Running until told to stop, then exits promptly on SIGTERM (a stop in a test
        // needs no grace-window clock advance).
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(clock.clone()),
        Arc::new(FakeTrustRepo::new()),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    let sup = Arc::new(Supervisor::new(&ports, bus.clone()));
    Setup {
        sup,
        clock,
        bus,
        rx,
    }
}

pub(crate) fn terminal(sup: &Supervisor) -> ProcessId {
    sup.register(Registration::launched(
        PROJECT,
        ProcessKind::Terminal,
        "shell",
        SpawnSpec {
            command: "sleep 60".into(),
            working_dir: PathBuf::from("/"),
            env: BTreeMap::new(),
            size: PtySize::default(),
        },
    ))
}

/// Drives a process to Running and returns its id.
pub(crate) async fn running_process(s: &mut Setup) -> ProcessId {
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_all(&mut s.rx, &[id], ProcStatus::Running).await;
    id
}

pub(crate) fn view_of(sup: &Supervisor, id: ProcessId) -> ProcessView {
    sup.snapshot()
        .into_iter()
        .find(|view| view.id == id)
        .expect("process is registered")
}
