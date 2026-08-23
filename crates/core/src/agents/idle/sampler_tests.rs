//! Behavioural tests for [`IdleSampler`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no real
//! time elapsed: an agent that streams output then goes quiet is classified Working then Idle,
//! an agent that never runs is not classified at all, and a lineage group whose processes have
//! all left the registry is reclaimed even when no agent is tracked.

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

/// How many times each round yields to the runtime, so the spawned sampler and the process
/// actors get to run before the round's assertion.
const YIELDS: usize = 16;

/// How many advance-and-settle rounds a wait gives what it is waiting for before it fails the test.
const MAX_ADVANCE_ROUNDS: usize = 400;

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

/// A launch spec for a registration that is never started, so the command never runs.
fn spec(command: &str) -> SpawnSpec {
    SpawnSpec {
        command: command.into(),
        working_dir: PathBuf::from("/"),
        env: BTreeMap::new(),
        size: PtySize::default(),
    }
}

impl Setup {
    /// Registers an agent process and begins tracking its idle activity.
    fn agent(&self) -> ProcessId {
        let id = self.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Agent,
            "Claude",
            spec("claude"),
        ));
        self.tracker.track(id, AgentKind::Claude);
        id
    }

    /// Registers a plain command process — the shape a lead agent spawns, which lands in the
    /// lineage map but never in the idle tracker.
    fn command(&self) -> ProcessId {
        self.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Command,
            "Build",
            spec("build"),
        ))
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

/// Yields to the runtime repeatedly so the spawned sampler and the process actors can run.
async fn settle() {
    for _ in 0..YIELDS {
        tokio::task::yield_now().await;
    }
}

/// Advances the mock clock and yields until `pred` holds, or fails `what` after a bounded
/// number of rounds — for the sampler's tick-driven effects on state rather than on events.
async fn advance_until<F: Fn() -> bool>(clock: &MockClock, what: &str, pred: F) {
    for _ in 0..MAX_ADVANCE_ROUNDS {
        clock.advance(ADVANCE_STEP);
        settle().await;
        if pred() {
            return;
        }
    }
    panic!("{what} did not happen within the budget");
}

/// Advances the mock clock and yields repeatedly until an `AgentActivityChanged` to `want`
/// arrives for `id`, or fails after a bounded number of rounds.
async fn wait_for_activity(
    rx: &mut broadcast::Receiver<DomainEvent>,
    clock: &MockClock,
    id: ProcessId,
    want: AgentActivity,
) {
    for _ in 0..MAX_ADVANCE_ROUNDS {
        clock.advance(ADVANCE_STEP);
        settle().await;
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
        settle().await;
    }
    let mut classified = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, DomainEvent::AgentActivityChanged { .. }) {
            classified = true;
        }
    }
    assert!(!classified, "a stopped agent is not classified");
}

#[tokio::test]
async fn a_running_agent_that_has_not_emitted_provider_evidence_is_not_idle() {
    let mut s = setup(FakeSpawner::streams_then_stays_alive(Vec::new()));
    let id = s.agent();
    s.sup.start(id).expect("start");
    wait_for_running(&mut s.rx, id).await;
    s.spawn_sampler();

    for _ in 0..5 {
        s.clock.advance(SAMPLE_INTERVAL);
        settle().await;
    }

    assert_eq!(s.tracker.activity(id), None);
    while let Ok(event) = s.rx.try_recv() {
        assert!(!matches!(
            event,
            DomainEvent::AgentActivityChanged {
                id: got,
                state: AgentActivity::Idle,
            } if got == id
        ));
    }
}

#[tokio::test]
async fn a_dead_lineage_group_is_reclaimed_on_a_tick_with_no_agent_tracked() {
    let s = setup(FakeSpawner::exits_on_kill());
    let lead = s.agent();
    let worker = s.command();
    s.lineage.record(worker, lead);
    s.spawn_sampler();

    // The lead leaves the registry first. Its edge survives, because it still connects the
    // live worker to its authorization root, while the tracker prunes down to empty.
    s.sup.close(lead).await.expect("close the lead");
    advance_until(&s.clock, "the tracker to forget the lead", || {
        s.tracker.tracked().is_empty()
    })
    .await;
    assert_eq!(s.lineage.edges(), vec![(worker, lead)]);

    // Now the worker goes too. No agent is tracked, so nothing is left to classify — but the
    // whole group is dead and the lineage map must still be reclaimed.
    s.sup.close(worker).await.expect("close the worker");
    advance_until(&s.clock, "the dead lineage group to be reclaimed", || {
        s.lineage.edges().is_empty()
    })
    .await;
}
