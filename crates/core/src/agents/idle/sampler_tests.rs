//! Behavioural tests for [`IdleSampler`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no real
//! time elapsed: an agent that streams output then goes quiet is classified Working then Idle,
//! and an agent that never runs is not classified at all.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::agents::{AgentActivity, AgentKind, AgentLineage, IdleSampler, IdleTracker};
use crate::composition::CorePorts;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{PtySize, SpawnSpec};
use crate::process::ProcStatus;
use crate::process::ProcessKind;
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

use super::SAMPLE_INTERVAL;

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// A clock step generous enough to fire whichever single timer is pending each round — a
/// sample interval or a supervision backoff — so the sampler is driven without knowing the
/// backoff bound.
const ADVANCE_STEP: Duration = Duration::from_secs(10);

struct Setup {
    sup: Arc<Supervisor>,
    tracker: Arc<IdleTracker>,
    lineage: Arc<AgentLineage>,
    clock: MockClock,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
}

fn setup(spawner: FakeSpawner) -> Setup {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let clock = MockClock::new();
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(clock.clone()),
        Arc::new(FakeTrustRepo::new()),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
    Setup {
        sup,
        tracker: Arc::new(IdleTracker::new()),
        lineage: Arc::new(AgentLineage::new()),
        clock,
        bus,
        rx,
    }
}

impl Setup {
    /// Registers an agent process and begins tracking its idle activity.
    fn agent(&self) -> ProcessId {
        let id = self.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Agent,
            "Claude",
            SpawnSpec {
                command: "claude".into(),
                working_dir: PathBuf::from("/"),
                env: BTreeMap::new(),
                size: PtySize::default(),
            },
        ));
        self.tracker.track(id, AgentKind::Claude);
        id
    }

    /// Spawns the sampler over this setup's supervisor, tracker, clock, and bus.
    fn spawn_sampler(&self) {
        tokio::spawn(
            IdleSampler::new(
                Arc::new(self.clock.clone()),
                self.tracker.clone(),
                self.lineage.clone(),
                self.bus.clone(),
                Arc::downgrade(&self.sup),
            )
            .run(),
        );
    }
}

async fn wait_for_running(rx: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { id: got, to, .. })
                if got == id && to == ProcStatus::Running =>
            {
                return
            }
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

/// Advances the mock clock and yields repeatedly until an `AgentActivityChanged` to `want`
/// arrives for `id`, or fails after a bounded number of rounds.
async fn wait_for_activity(
    rx: &mut broadcast::Receiver<DomainEvent>,
    clock: &MockClock,
    id: ProcessId,
    want: AgentActivity,
) {
    for _ in 0..400 {
        clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = rx.try_recv() {
            if let DomainEvent::AgentActivityChanged { id: got, state } = event {
                if got == id && state == want {
                    return;
                }
            }
        }
    }
    panic!("no AgentActivityChanged to {want:?} for {id:?} within the budget");
}

#[tokio::test]
async fn an_agent_that_outputs_then_quiets_goes_working_then_idle() {
    let mut s = setup(FakeSpawner::streams_then_stays_alive(vec![
        b"thinking about it...\n".to_vec(),
    ]));
    let id = s.agent();
    s.sup.start(id).expect("start");
    wait_for_running(&mut s.rx, id).await;
    s.spawn_sampler();

    // Output appeared, so the agent is Working; once it goes quiet it settles to Idle.
    wait_for_activity(&mut s.rx, &s.clock, id, AgentActivity::Working).await;
    wait_for_activity(&mut s.rx, &s.clock, id, AgentActivity::Idle).await;
}

#[tokio::test]
async fn an_agent_that_never_runs_is_not_classified() {
    let s = setup(FakeSpawner::exits_on_kill());
    let _id = s.agent(); // tracked but never started — stays Stopped
    s.spawn_sampler();

    let mut rx = s.bus.subscribe();
    for _ in 0..5 {
        s.clock.advance(SAMPLE_INTERVAL);
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }
    let mut classified = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, DomainEvent::AgentActivityChanged { .. }) {
            classified = true;
        }
    }
    assert!(!classified, "a stopped agent is not classified");
}
