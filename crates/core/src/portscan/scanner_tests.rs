//! Behavioural tests for [`PortScanner`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no
//! real time and no real `/proc` read.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{CorePorts, PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{FakePortProbe, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

use super::PortScanner;

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// A clock step generous enough to fire whichever single timer is pending each round.
const ADVANCE_STEP: Duration = Duration::from_secs(10);

struct Setup {
    sup: Arc<Supervisor>,
    clock: MockClock,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
}

fn setup() -> Setup {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let clock = MockClock::new();
    let ports = CorePorts::builder(
        // Stays Running until told to stop, then exits promptly on SIGTERM (the stop test
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

fn terminal(sup: &Supervisor) -> ProcessId {
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

async fn wait_for(rx: &mut broadcast::Receiver<DomainEvent>, id: ProcessId, target: ProcStatus) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { id: got, to, .. })
                if got == id && to == target =>
            {
                return
            }
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

async fn next_ports_changed(
    rx: &mut broadcast::Receiver<DomainEvent>,
    clock: &MockClock,
    id: ProcessId,
) -> Vec<u16> {
    for _ in 0..200 {
        clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = rx.try_recv() {
            if let DomainEvent::PortsChanged { id: got, ports } = event {
                if got == id {
                    return ports;
                }
            }
        }
    }
    panic!("no PortsChanged for {id:?} within the budget");
}

fn ports_in_snapshot(sup: &Supervisor, id: ProcessId) -> Vec<u16> {
    sup.snapshot()
        .into_iter()
        .find(|view| view.id == id)
        .map(|view| view.ports)
        .expect("process is registered")
}

#[tokio::test]
async fn a_running_process_has_its_ports_discovered_then_announced_once() {
    let mut s = setup();
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_for(&mut s.rx, id, ProcStatus::Running).await;

    let probe = FakePortProbe::returning(vec![8080]);
    tokio::spawn(
        PortScanner::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    // Discovery announces the port and reflects it on the read model.
    assert_eq!(
        next_ports_changed(&mut s.rx, &s.clock, id).await,
        vec![8080]
    );
    assert_eq!(ports_in_snapshot(&s.sup, id), vec![8080]);

    // A later scan with the same ports announces nothing — the read model never churns.
    for _ in 0..3 {
        s.clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
    }
    let mut churned = false;
    while let Ok(event) = s.rx.try_recv() {
        if matches!(event, DomainEvent::PortsChanged { id: got, .. } if got == id) {
            churned = true;
        }
    }
    assert!(
        !churned,
        "an unchanged scan announces no further PortsChanged"
    );
}

#[tokio::test]
async fn ports_clear_when_the_process_stops() {
    let mut s = setup();
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_for(&mut s.rx, id, ProcStatus::Running).await;

    let probe = FakePortProbe::returning(vec![5173]);
    tokio::spawn(
        PortScanner::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
    assert_eq!(
        next_ports_changed(&mut s.rx, &s.clock, id).await,
        vec![5173]
    );

    // Stopping the process ends its group, so its discovered ports are cleared.
    s.sup.stop(id);
    wait_for(&mut s.rx, id, ProcStatus::Stopped).await;
    assert!(
        ports_in_snapshot(&s.sup, id).is_empty(),
        "a stopped process lists no ports",
    );
}
