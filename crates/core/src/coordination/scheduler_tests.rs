//! Behavioural tests for [`TimerScheduler`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no real time
//! elapsed: a deadline timer fires and delivers its body as a fresh turn; a fire-when-idle-all
//! timer fires only once every watched process is idle; a process nobody started never satisfies
//! the wait while one that ran and ended does; a backstop fires a stuck wait; pausing suspends
//! firing; and a closing owner's timers are dropped.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::Notify;

use crate::agents::AgentActivity;
use crate::composition::CorePorts;
use crate::coordination::{FireCond, IdleMode, StoredTimer, TimerRepo, TimerStatus, Timers};
use crate::events::{DomainEvent, EventBus};
use crate::idle::{ObservedActivities, ObservedActivity};
use crate::ids::{ProcessId, ProjectId, TimerId};
use crate::ports::{PtySize, SpawnSpec};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::{Registration, Supervisor};
use crate::sync::lock;
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTimerRepo, FakeTrustRepo, MockClock};

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// How many times to yield to the runtime after an action, letting the spawned scheduler loop (and
/// any process actor) make progress before the assertion — the deterministic stand-in for waiting.
const YIELDS: usize = 64;

/// Stands in for the idle tracker (C4) the scheduler reads. The real one is fed by sampling an
/// agent's terminal, which these tests have no way to produce, so observations are recorded
/// directly — the values are the same ones the tracker would hold.
#[derive(Default)]
struct FakeIdle {
    observed: Mutex<HashMap<ProcessId, ObservedActivity>>,
}

impl FakeIdle {
    /// Records that `id` has been launched under classification, as the tracker does the moment
    /// an agent is launched.
    fn launched(&self, id: ProcessId) {
        lock(&self.observed).insert(id, ObservedActivity::launched());
    }

    /// Records one classification for `id`, as a sample of its terminal would.
    fn observe(&self, id: ProcessId, activity: AgentActivity) {
        lock(&self.observed)
            .entry(id)
            .or_insert_with(ObservedActivity::launched)
            .observe(activity);
    }
}

impl ObservedActivities for FakeIdle {
    fn observed_activity(&self, process: ProcessId) -> ObservedActivity {
        lock(&self.observed)
            .get(&process)
            .copied()
            .unwrap_or_default()
    }
}

struct Harness {
    sup: Arc<Supervisor>,
    timers: Timers,
    repo: Arc<FakeTimerRepo>,
    clock: MockClock,
    bus: EventBus,
    idle: Arc<FakeIdle>,
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
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
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
        idle: Arc::new(FakeIdle::default()),
    }
}

impl Harness {
    /// Registers and starts a long-lived agent process, recording its launch the way the agent
    /// launch path does, and returns its id once it is Running.
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
        self.idle.launched(id);
        id
    }

    /// Registers a command without ever starting it — a configured process nobody has launched,
    /// which rests in the same status as one that ran and exited.
    fn unstarted_process(&self) -> ProcessId {
        self.sup.register(Registration::launched(
            PROJECT,
            ProcessKind::Command,
            "api",
            SpawnSpec {
                command: "npm run dev".into(),
                working_dir: PathBuf::from("/"),
                env: BTreeMap::new(),
                size: PtySize::default(),
            },
        ))
    }

    /// Classifies `id` the way C4 does: recorded in the tracker every reader shares, and announced
    /// on the bus so the scheduler re-evaluates.
    fn classify(&self, id: ProcessId, activity: AgentActivity) {
        self.idle.observe(id, activity);
        self.bus.publish(DomainEvent::AgentActivityChanged {
            id,
            state: activity,
        });
    }

    fn spawn_scheduler(&self) {
        tokio::spawn(
            self.timers
                .scheduler(
                    self.bus.clone(),
                    Arc::downgrade(&self.sup),
                    self.idle.clone(),
                )
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
    let (spawner, delivered) = FakeSpawner::records_input();
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

    // Past the deadline it fires: claimed from the store and delivered to the owner as a fresh
    // submitted turn — the wake-reason header on its own line so the agent can tell why it woke,
    // the body beneath it, and the carriage return that submits it. Without that byte the body
    // sits in the agent's prompt as an unsent draft and the wake never happens. The header's
    // wording is covered by the `wake_reason_header` tests; here only the turn's shape is.
    advance_until(&h.clock, Duration::from_secs(10), || {
        String::from_utf8_lossy(&delivered.to(owner)).contains("resume work")
    })
    .await;
    let turn = String::from_utf8(delivered.to(owner)).expect("utf-8 input");
    let (header, body) = turn
        .split_once('\n')
        .expect("a wake-reason header line above the body");
    assert!(
        header.starts_with(&format!("[Soloist timer #{}]", view.id)),
        "the woken agent is told which timer fired: {header:?}"
    );
    assert_eq!(
        body, "resume work\r",
        "the body is delivered whole and submitted by a trailing carriage return"
    );
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
    // Both take up their turn — the stream a working agent produces before it settles.
    for worker in [first, second] {
        h.classify(worker, AgentActivity::Working);
    }

    // One worker idle is not enough for an all-timer.
    h.classify(first, AgentActivity::Idle);
    settle().await;
    assert!(h.armed(view.id), "one of two idle does not satisfy `all`");

    // Both idle: the timer fires.
    h.classify(second, AgentActivity::Idle);
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

    // One worker takes up its turn and finishes it; that alone satisfies an any-timer.
    h.classify(second, AgentActivity::Working);
    h.classify(second, AgentActivity::Idle);
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
async fn a_watched_worker_that_has_not_begun_a_turn_does_not_fire_the_timer() {
    // A worker's terminal is quiet while its CLI starts up, so it is classified Idle before it has
    // done anything. That quiet is not a finished turn: the timer must keep waiting until the
    // worker has taken up its work and settled again, or the lead is woken before the work exists.
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
            "worker finished".into(),
            vec![worker],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");

    h.classify(worker, AgentActivity::Idle);
    settle().await;
    assert!(
        h.armed(view.id),
        "a worker that has only ever been quiet has not finished anything"
    );

    // It takes up its turn and finishes it: now its quiet is the real thing.
    h.classify(worker, AgentActivity::Working);
    h.classify(worker, AgentActivity::Idle);
    settle_until(|| !h.armed(view.id)).await;
}

#[tokio::test]
async fn a_watched_process_that_was_never_started_does_not_fire_the_timer() {
    // A configured command nobody has launched rests in the same status as one that ran and
    // exited. It has done nothing, so it cannot have finished anything: a lead watching it beside
    // a worker must not be woken the instant it arms the timer, before the worker has begun.
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let unstarted = h.unstarted_process();
    h.spawn_scheduler();
    settle().await;
    assert!(
        h.sup
            .view(unstarted)
            .is_some_and(|view| !view.status.is_active()),
        "the watched command is registered and at rest, never having run"
    );

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "workers done".into(),
            vec![unstarted],
            IdleMode::Any,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");

    settle().await;
    assert!(
        h.armed(view.id),
        "a process that was never started has not finished; the wait continues"
    );
}

#[tokio::test]
async fn a_scheduler_that_never_saw_the_idle_transition_still_fires() {
    // The scheduler reads the shared idle state instead of folding the event stream into its own,
    // so a transition it was not listening for — announced before this loop started, or dropped
    // while the bus lagged — is not lost. A loop that folded the stream would come back from a
    // supervised restart with an empty map and hold the timer to its hour-long backstop.
    let h = harness(FakeSpawner::exits_on_kill());
    let owner = h.running_process().await;
    let worker = h.running_process().await;

    // The worker takes up its turn and finishes it with nothing subscribed to the bus.
    h.idle.observe(worker, AgentActivity::Working);
    h.idle.observe(worker, AgentActivity::Idle);

    h.spawn_scheduler();
    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "worker finished".into(),
            vec![worker],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");

    // No event and no clock advance: arming wakes the scheduler, which reads the state it never
    // witnessed and fires far before the backstop.
    settle_until(|| !h.armed(view.id)).await;
}

#[tokio::test]
async fn a_watched_process_that_exits_while_working_fires_the_timer() {
    // A worker that exits can do no more work, so the wait is over — even though the last activity
    // classified for it was `Working`, and even though it stays registered (its scrollback remains
    // readable). Without the status, the stale `Working` would hold the timer to its backstop.
    let h = harness(FakeSpawner::exits_on_terminate());
    let owner = h.running_process().await;
    let worker = h.running_process().await;
    h.spawn_scheduler();
    settle().await;

    let view = h
        .timers
        .set_when_idle(
            PROJECT,
            owner,
            "worker finished".into(),
            vec![worker],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("set");
    h.classify(worker, AgentActivity::Working);
    settle().await;
    assert!(h.armed(view.id), "a working worker does not fire the timer");

    // The worker exits. No clock advance, so the hour-long backstop cannot be what fires it.
    h.sup.stop(worker);
    settle_until(|| h.sup.view(worker).map(|view| view.status) == Some(ProcStatus::Stopped)).await;
    assert!(
        h.sup.view(worker).is_some(),
        "the exited worker is still registered, so only its status ends the wait"
    );
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
    h.classify(worker, AgentActivity::Working);
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
