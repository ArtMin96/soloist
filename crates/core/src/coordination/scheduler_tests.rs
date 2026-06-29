//! Behavioural tests for [`TimerScheduler`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no real time
//! elapsed: a deadline timer fires and delivers its body as a fresh turn; a fire-when-idle-all
//! timer fires only once every watched process is idle; a backstop fires a stuck wait; pausing
//! suspends firing; and a closing owner's timers are dropped.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::Notify;

use crate::agents::AgentActivity;
use crate::coordination::{FireCond, IdleMode, StoredTimer, TimerRepo, TimerStatus, Timers};
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId, TimerId};
use crate::ports::{CorePorts, PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::{Registration, Supervisor};
use crate::sync::lock;
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTimerRepo, FakeTrustRepo, MockClock};

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// How many times to yield to the runtime after an action, letting the spawned scheduler loop (and
/// any process actor) make progress before the assertion — the deterministic stand-in for waiting.
const YIELDS: usize = 64;

struct Harness {
    sup: Arc<Supervisor>,
    timers: Timers,
    repo: Arc<FakeTimerRepo>,
    clock: MockClock,
    bus: EventBus,
}

fn harness(spawner: FakeSpawner) -> Harness {
    let bus = EventBus::new(256);
    let clock = MockClock::new();
    let repo = Arc::new(FakeTimerRepo::new());
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(clock.clone()),
        Arc::new(FakeTrustRepo::new()),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    let sup = Arc::new(Supervisor::new(&ports, bus.clone()));
    let timers = Timers::new(
        repo.clone(),
        Arc::new(clock.clone()),
        Arc::new(Notify::new()),
    );
    Harness {
        sup,
        timers,
        repo,
        clock,
        bus,
    }
}

impl Harness {
    /// Registers and starts a long-lived agent process, returning its id once it is Running.
    async fn running_process(&self) -> ProcessId {
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
        self.sup.start(id).expect("start");
        wait_for_running(&mut self.bus.subscribe(), id).await;
        id
    }

    fn spawn_scheduler(&self) {
        tokio::spawn(
            self.timers
                .scheduler(self.bus.clone(), Arc::downgrade(&self.sup))
                .run(),
        );
    }

    /// Whether the timer is still armed (counting down) — the observable of "not yet fired".
    fn armed(&self, id: TimerId) -> bool {
        self.repo
            .armed()
            .expect("armed")
            .iter()
            .any(|timer| timer.id == id)
    }

    /// Whether the timer still exists at all (armed or paused), regardless of state.
    fn exists(&self, owner: ProcessId, id: TimerId) -> bool {
        self.repo
            .list(owner)
            .expect("list")
            .iter()
            .any(|timer| timer.id == id)
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

/// Yields to the runtime repeatedly so the spawned scheduler (and process actors) can run.
async fn settle() {
    for _ in 0..YIELDS {
        tokio::task::yield_now().await;
    }
}

/// Advances the clock and yields until `pred` holds, or fails after a bounded budget — for the
/// deadline-driven paths.
async fn advance_until<F: Fn() -> bool>(clock: &MockClock, step: Duration, pred: F) {
    for _ in 0..400 {
        clock.advance(step);
        settle().await;
        if pred() {
            return;
        }
    }
    panic!("condition not met within the budget");
}

/// Advances the clock and drains the event stream until one satisfying `pred` is seen, returning
/// whether it arrived within the budget — for asserting a deadline-driven emission.
async fn advance_until_event(
    clock: &MockClock,
    step: Duration,
    rx: &mut broadcast::Receiver<DomainEvent>,
    pred: impl Fn(&DomainEvent) -> bool,
) -> bool {
    for _ in 0..400 {
        clock.advance(step);
        settle().await;
        loop {
            match rx.try_recv() {
                Ok(event) if pred(&event) => return true,
                Ok(_) => continue,
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Closed) => return false,
            }
        }
    }
    false
}

/// Yields until `pred` holds without advancing time, or fails after a bounded budget — for the
/// event-driven (idle/removal) paths, where advancing the clock could trip an unrelated backstop.
async fn settle_until<F: Fn() -> bool>(pred: F) {
    for _ in 0..400 {
        settle().await;
        if pred() {
            return;
        }
    }
    panic!("condition not met within the budget");
}

#[tokio::test]
async fn an_at_timer_fires_at_its_deadline_and_delivers_the_body_as_a_fresh_turn() {
    let (spawner, recorder) = FakeSpawner::records_input();
    let h = harness(spawner);
    let owner = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set(
            PROJECT,
            owner,
            "resume work".into(),
            Some(Duration::from_secs(5)),
        )
        .expect("set");

    // Before the deadline it has not fired.
    settle().await;
    assert!(h.armed(view.id), "the timer waits until its deadline");

    // Past the deadline it fires: claimed from the store and delivered to the owner with the
    // wake-reason header prepended and a trailing carriage return, so the agent receives it as a
    // submitted fresh turn and can tell why it woke (the header format is tested separately via
    // `wake_reason_header` — here we only assert the body text is present).
    advance_until(&h.clock, Duration::from_secs(10), || {
        String::from_utf8_lossy(&lock(&recorder)).contains("resume work")
    })
    .await;
    assert!(!h.exists(owner, view.id), "a fired timer is gone");
}

#[tokio::test]
async fn firing_a_timer_emits_a_timer_fired_event() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    h.spawn_scheduler();
    settle().await;
    // Subscribe after setup, so the only events seen are the timer's.
    let mut rx = h.bus.subscribe();

    let view = h
        .timers
        .set(PROJECT, owner, "go".into(), Some(Duration::from_secs(5)))
        .expect("set");

    // Past the deadline the scheduler claims and fires the timer, announcing it on the bus so the
    // wake-cycle UI can surface that the lead woke.
    let fired = advance_until_event(&h.clock, Duration::from_secs(10), &mut rx, |event| {
        matches!(event, DomainEvent::TimerFired { owner: o, id } if *o == owner && *id == view.id)
    })
    .await;
    assert!(
        fired,
        "the scheduler emits TimerFired for the timer it fired"
    );
}

#[tokio::test]
async fn fire_when_idle_all_fires_only_when_every_watched_process_is_idle() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let first = h.running_process().await;
    let second = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "all done".into(),
            vec![first, second],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");
    settle().await;
    assert!(h.armed(view.id), "running workers are not idle yet");

    // One worker idle is not enough for an all-timer.
    h.bus.publish(DomainEvent::AgentActivityChanged {
        id: first,
        state: AgentActivity::Idle,
    });
    settle().await;
    assert!(h.armed(view.id), "one of two idle does not satisfy `all`");

    // Both idle: the timer fires.
    h.bus.publish(DomainEvent::AgentActivityChanged {
        id: second,
        state: AgentActivity::Idle,
    });
    settle_until(|| !h.armed(view.id)).await;
}

#[tokio::test]
async fn fire_when_idle_any_fires_as_soon_as_one_watched_process_is_idle() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let first = h.running_process().await;
    let second = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "one done".into(),
            vec![first, second],
            IdleMode::Any,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");
    settle().await;
    assert!(h.armed(view.id));

    h.bus.publish(DomainEvent::AgentActivityChanged {
        id: second,
        state: AgentActivity::Idle,
    });
    settle_until(|| !h.armed(view.id)).await;
}

#[tokio::test]
async fn a_watched_process_absent_from_the_registry_counts_as_idle_and_fires() {
    // A watched worker that has exited (is not in the registry) can no longer work, so it counts as
    // idle: the timer fires without ever seeing an idle event and without the backstop elapsing.
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let gone = ProcessId::from_raw(9999); // never registered → not in the supervisor
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "all done".into(),
            vec![gone],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");

    // No event published and no clock advance: arming wakes the scheduler, which sees the absent
    // process as idle and fires at once — far before the hour-long backstop.
    settle_until(|| !h.armed(view.id)).await;
}

#[tokio::test]
async fn a_non_idle_transition_does_not_fire_a_fire_when_idle_timer() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let worker = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "go".into(),
            vec![worker],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");

    // A Working transition is not idle — the timer keeps waiting.
    h.bus.publish(DomainEvent::AgentActivityChanged {
        id: worker,
        state: AgentActivity::Working,
    });
    settle().await;
    assert!(h.armed(view.id), "a working worker does not fire the timer");
}

#[tokio::test]
async fn the_max_wait_backstop_fires_even_if_no_process_goes_idle() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let worker = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "give up".into(),
            vec![worker],
            IdleMode::All,
            Some(Duration::from_secs(3)),
        )
        .expect("set");
    settle().await;
    assert!(h.armed(view.id));

    // The worker never goes idle; the backstop fires the timer anyway.
    advance_until(&h.clock, Duration::from_secs(5), || !h.armed(view.id)).await;
}

#[tokio::test]
async fn a_paused_timer_does_not_fire_at_its_deadline_until_resumed() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;

    let view = h
        .timers
        .set(PROJECT, owner, "ping".into(), Some(Duration::from_secs(5)))
        .expect("set");
    assert!(h.timers.pause(view.id, owner).expect("pause"));
    h.spawn_scheduler();
    settle().await;

    // Past the original deadline, the paused timer still has not fired.
    h.clock.advance(Duration::from_secs(60));
    settle().await;
    assert!(
        h.exists(owner, view.id) && !h.armed(view.id),
        "a paused timer is retained but never fired"
    );

    // Resuming re-arms it; it then fires.
    assert!(h.timers.resume(view.id, owner).expect("resume"));
    advance_until(&h.clock, Duration::from_secs(10), || {
        !h.exists(owner, view.id)
    })
    .await;
}

#[tokio::test]
async fn closing_the_owner_drops_its_timers() {
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set(
            PROJECT,
            owner,
            "ping".into(),
            Some(Duration::from_secs(3600)),
        )
        .expect("set");
    settle().await;
    assert!(h.exists(owner, view.id));

    // The owner closes: the scheduler sees the removal and drops the timers it owned.
    h.bus.publish(DomainEvent::ProcessRemoved { id: owner });
    settle_until(|| !h.exists(owner, view.id)).await;
}

/// A minimal stored timer with the given fire condition, for the pure wake-reason header tests.
fn stored_timer(id: u64, fire: FireCond) -> StoredTimer {
    StoredTimer {
        id: TimerId::from_raw(id),
        project: PROJECT,
        owner: ProcessId::from_raw(1),
        body: "resume".into(),
        fire,
        deadline_unix_millis: 1_000,
        status: TimerStatus::Armed,
        remaining_on_pause_millis: None,
    }
}

#[test]
fn the_wake_reason_header_names_a_scheduled_delivery_for_an_at_timer() {
    let timer = stored_timer(3, FireCond::At);
    assert_eq!(
        super::wake_reason_header(&timer, false),
        "[Soloist timer #3] scheduled delivery"
    );
}

#[test]
fn the_wake_reason_header_distinguishes_all_idle_from_the_backstop() {
    let watched = vec![ProcessId::from_raw(2), ProcessId::from_raw(3)];
    let timer = stored_timer(4, FireCond::WhenIdleAll { watched });
    assert_eq!(
        super::wake_reason_header(&timer, false),
        "[Soloist timer #4] all 2 watched agents are idle"
    );
    assert_eq!(
        super::wake_reason_header(&timer, true),
        "[Soloist timer #4] max-wait backstop elapsed (when-all-idle, 2 watched)"
    );
}

#[test]
fn the_wake_reason_header_distinguishes_any_idle_from_the_backstop() {
    let watched = vec![ProcessId::from_raw(9)];
    let timer = stored_timer(5, FireCond::WhenIdleAny { watched });
    assert_eq!(
        super::wake_reason_header(&timer, false),
        "[Soloist timer #5] a watched agent is idle (any-idle condition met)"
    );
    assert_eq!(
        super::wake_reason_header(&timer, true),
        "[Soloist timer #5] max-wait backstop elapsed (when-any-idle, 1 watched)"
    );
}
