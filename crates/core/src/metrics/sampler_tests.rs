//! Behavioural tests for [`MetricsSampler`], kept out of the implementation file. They
//! drive a real [`Supervisor`] over fakes and the mock clock, so timing is deterministic
//! with no real time elapsed and no real OS read.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::composition::CorePorts;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{FakeMetricsProbe, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

use super::{MetricsSampler, HEARTBEAT_SAMPLES, SAMPLE_INTERVAL};

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// A clock step generous enough to fire whichever single timer is pending each round — a
/// sample interval or a restart backoff — so the sampler is driven without knowing the
/// supervision backoff bound.
const ADVANCE_STEP: Duration = Duration::from_secs(10);

/// A running supervisor plus the bus the sampler publishes on and the clock it ticks on
/// — a minimal composition for sampler tests (the supervisor's own harness is private to
/// that module, and these tests need the shared bus exposed).
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
        // A long-lived fake child so a started process stays Running with a recorded pgid
        // (424242) for the sampler to target.
        Arc::new(FakeSpawner::exits_on_kill()),
        Arc::new(clock.clone()),
        Arc::new(FakeTrustRepo::new()),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
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

/// Advances the mock clock and yields repeatedly until a `MetricsTick` for `id` arrives,
/// or fails after a bounded number of rounds. Each round fires whatever single timer is
/// currently pending (the tick interval or a restart backoff) and lets the spawned tasks
/// progress, so the sampler is driven deterministically with no real time.
async fn next_metrics_tick(
    rx: &mut broadcast::Receiver<DomainEvent>,
    clock: &MockClock,
    id: ProcessId,
) -> (f32, u64) {
    for _ in 0..200 {
        clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = rx.try_recv() {
            if let DomainEvent::MetricsTick {
                id: got,
                cpu_pct,
                rss,
            } = event
            {
                if got == id {
                    return (cpu_pct, rss);
                }
            }
        }
    }
    panic!("no MetricsTick for {id:?} within the budget");
}

#[tokio::test]
async fn a_running_process_is_sampled_each_tick() {
    let mut s = setup();
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_for_running(&mut s.rx, id).await;

    let probe = FakeMetricsProbe::returning(12.5, 4096);
    tokio::spawn(
        MetricsSampler::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe.clone()),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    let (cpu, rss) = next_metrics_tick(&mut s.rx, &s.clock, id).await;
    assert_eq!(cpu, 12.5);
    assert_eq!(rss, 4096);
    assert!(probe.calls() >= 1, "the probe was sampled");
}

#[tokio::test]
async fn the_sampler_restarts_itself_after_a_panic() {
    // The probe panics on its first sample, then behaves — proving the sampling loop is
    // panic-isolated and restarted, so monitoring recovers on its own.
    let mut s = setup();
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_for_running(&mut s.rx, id).await;

    let probe = FakeMetricsProbe::returning(7.0, 2048).panic_once();
    tokio::spawn(
        MetricsSampler::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe.clone()),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    // A tick still arrives despite the first sample panicking, and the probe was called
    // more than once — the loop was sampled, died, and resumed.
    let (cpu, rss) = next_metrics_tick(&mut s.rx, &s.clock, id).await;
    assert_eq!((cpu, rss), (7.0, 2048));
    assert!(probe.calls() >= 2, "panicked once, then sampled again");
}

#[tokio::test]
async fn an_unchanged_reading_is_suppressed_between_heartbeats() {
    // A steady process holds a constant reading; after it publishes once, the next few identical
    // samples (fewer than a heartbeat window) are suppressed — the sampler keeps polling but does
    // not churn the UI with unchanged ticks.
    let mut s = setup();
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_for_running(&mut s.rx, id).await;

    let probe = FakeMetricsProbe::returning(3.0, 512);
    tokio::spawn(
        MetricsSampler::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe.clone()),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    // The first reading is published.
    assert_eq!(next_metrics_tick(&mut s.rx, &s.clock, id).await, (3.0, 512));

    // Drive several more samples (fewer than a heartbeat window) with the same reading and confirm
    // none is re-emitted. Progress is measured by the probe's sample count, not wall-clock rounds,
    // so scheduler contention under a parallel test run only slows the test — it can never make it
    // observe a false "suppressed" from a sample that never ran.
    let target = probe.calls() + (HEARTBEAT_SAMPLES as usize) / 2;
    let mut rx = s.bus.subscribe();
    let mut re_emitted = false;
    for _ in 0..500 {
        if probe.calls() >= target {
            break;
        }
        s.clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = rx.try_recv() {
            if matches!(event, DomainEvent::MetricsTick { id: got, .. } if got == id) {
                re_emitted = true;
            }
        }
    }
    assert!(probe.calls() >= target, "the probe kept sampling");
    assert!(
        !re_emitted,
        "an unchanged reading is suppressed within a heartbeat window"
    );
}

#[tokio::test]
async fn a_steady_reading_is_re_emitted_as_a_heartbeat() {
    // A steady process must not fall silent forever: the UI has no snapshot to seed from, so a
    // subscriber that mounts (or reloads) after the reading last moved would show a blank reading.
    // After the heartbeat window the unchanged reading is re-published so any such subscriber
    // repopulates.
    let mut s = setup();
    let id = terminal(&s.sup);
    s.sup.start(id).expect("start");
    wait_for_running(&mut s.rx, id).await;

    let probe = FakeMetricsProbe::returning(3.0, 512);
    tokio::spawn(
        MetricsSampler::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe.clone()),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    assert_eq!(next_metrics_tick(&mut s.rx, &s.clock, id).await, (3.0, 512));

    // Drive past a full heartbeat window with the same reading and confirm it is re-published. A
    // fresh subscriber (which missed the first publish) must still receive the reading — the
    // property the heartbeat guarantees. Progress is measured by the probe's sample count (so a
    // parallel run's scheduler contention only slows the test), with a generous round budget.
    let target = probe.calls() + HEARTBEAT_SAMPLES as usize + 2;
    let mut rx = s.bus.subscribe();
    let mut re_emitted = false;
    for _ in 0..1000 {
        if re_emitted || probe.calls() >= target {
            break;
        }
        s.clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = rx.try_recv() {
            if matches!(event, DomainEvent::MetricsTick { id: got, .. } if got == id) {
                re_emitted = true;
            }
        }
    }
    assert!(
        re_emitted,
        "a steady reading is re-published as a heartbeat"
    );
}

#[tokio::test]
async fn a_process_with_no_live_group_is_not_sampled() {
    // A registered-but-never-started process has no recorded group, so the sampler targets
    // nothing and emits no tick (and never calls the probe with it).
    let s = setup();
    let _id = terminal(&s.sup);
    let probe = FakeMetricsProbe::returning(1.0, 1);
    tokio::spawn(
        MetricsSampler::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe.clone()),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    let mut rx = s.bus.subscribe();
    for _ in 0..5 {
        s.clock.advance(SAMPLE_INTERVAL);
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }
    assert!(
        matches!(rx.try_recv(), Err(broadcast::error::TryRecvError::Empty)),
        "no events for an unstarted process",
    );
}
