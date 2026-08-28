//! Behavioural tests for [`MetricsSampler`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so every timer the sampler waits on fires
//! when the test says so and no OS is read.
//!
//! Every effect the sampler produces crosses the blocking thread pool — its OS read runs under
//! `run_blocking` — so the wake that arms its next timer arrives from another thread. Waits here
//! therefore advance the clock on a short *real* interval under a wall-clock ceiling
//! ([`drive_until`]): a budget of cooperative `yield_now`s cannot order a cross-thread wake, and
//! spans so little real time that it expires on a merely slow sample rather than a broken one.
//!
//! [`a_process_with_no_live_group_is_not_sampled`] keeps a yield budget, because its assertion is
//! that nothing happens: there is no effect to await, and a budget that runs short there only
//! weakens the assertion instead of failing the test.

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

/// How much real time [`drive_until`] leaves between advances, for the sample it just woke to
/// cross the blocking pool and arm the sampler's next timer. Short, so a test that is going to
/// progress does so in milliseconds.
const DRIVE_STEP: Duration = Duration::from_millis(5);

/// The wall-clock ceiling on a drive loop, so an effect that is never going to arrive fails the
/// test loudly instead of parking it. Far above the milliseconds these waits take, and far below
/// a CI job's timeout.
const DRIVE_LIMIT: Duration = Duration::from_secs(5);

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

/// Advances the mock clock until `settled` holds, failing after [`DRIVE_LIMIT`]. Each round fires
/// whichever single timer is pending — the sample interval, or the backoff a supervised restart
/// waits out — then leaves [`DRIVE_STEP`] of real time for the woken sample to run on the blocking
/// pool and arm the next timer. An advance that lands before the sampler has armed costs one
/// round, since the sampler then arms from the new reading and the following advance reaches it;
/// the ceiling being wall-clock is what makes that safe, as a stalled round spends the budget in
/// proportion to how long it stalls rather than all at once.
async fn drive_until(what: &str, clock: &MockClock, mut settled: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + DRIVE_LIMIT;
    while !settled() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}",
        );
        clock.advance(ADVANCE_STEP);
        tokio::time::sleep(DRIVE_STEP).await;
    }
}

/// Drains everything queued on `rx`, returning the reading of the last `MetricsTick` for `id`
/// among it.
fn drain_tick(rx: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) -> Option<(f32, u64)> {
    let mut reading = None;
    while let Ok(event) = rx.try_recv() {
        if let DomainEvent::MetricsTick {
            id: got,
            cpu_pct,
            rss,
        } = event
        {
            if got == id {
                reading = Some((cpu_pct, rss));
            }
        }
    }
    reading
}

/// Drives the sampler until the probe has taken `target` samples or a `MetricsTick` for `id` is
/// published, whichever comes first, and reports whether one was — how both emit-on-change tests
/// observe a window's worth of sampling.
///
/// The sample count is read before each drain, so the drain that decides the answer is the one
/// taken after the count reached its target: the sampler publishes a sample's reading before it
/// starts the next, so a target that has been reached implies every tick it covers is already
/// queued.
async fn ticked_within_samples(
    clock: &MockClock,
    rx: &mut broadcast::Receiver<DomainEvent>,
    probe: &FakeMetricsProbe,
    id: ProcessId,
    target: usize,
) -> bool {
    let mut ticked = false;
    drive_until(
        &format!("the probe to reach {target} samples or tick for {id:?}"),
        clock,
        || {
            let sampled_enough = probe.calls() >= target;
            ticked |= drain_tick(rx, id).is_some();
            ticked || sampled_enough
        },
    )
    .await;
    ticked
}

/// Drives the sampler until a `MetricsTick` for `id` arrives, returning its reading.
async fn next_metrics_tick(
    rx: &mut broadcast::Receiver<DomainEvent>,
    clock: &MockClock,
    id: ProcessId,
) -> (f32, u64) {
    let mut reading = None;
    drive_until(&format!("a MetricsTick for {id:?}"), clock, || {
        reading = drain_tick(rx, id);
        reading.is_some()
    })
    .await;
    reading.expect("drive_until returns only once a tick has been drained")
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
    // none is re-emitted. Progress is measured by the probe's sample count, so scheduler contention
    // under a parallel test run only slows the test — it can never make it observe a false
    // "suppressed" from a sample that never ran.
    let target = probe.calls() + (HEARTBEAT_SAMPLES as usize) / 2;
    let mut rx = s.bus.subscribe();
    assert!(
        !ticked_within_samples(&s.clock, &mut rx, &probe, id, target).await,
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
    // property the heartbeat guarantees. Progress is measured by the probe's sample count, so a
    // parallel run's scheduler contention only slows the test.
    let target = probe.calls() + HEARTBEAT_SAMPLES as usize + 2;
    let mut rx = s.bus.subscribe();
    assert!(
        ticked_within_samples(&s.clock, &mut rx, &probe, id, target).await,
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
