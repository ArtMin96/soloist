//! Behavioural tests for [`AgentMailboxReactor`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no real time
//! elapsed: an agent that reaches `Idle` is woken at once, an agent the classifier never types is
//! woken by the backstop and not before, that backstop delivers one wake rather than a recurring
//! poke, and a submission the agent refuses at the backstop is retried rather than lost.

use tokio::sync::broadcast;

use super::super::vocabulary::MAX_WAKE_WAIT;
use super::*;
use crate::composition::CorePorts;
use crate::coordination::{AgentMessageKind, AgentMessageOutcome};
use crate::ids::ProjectId;
use crate::process::ProcStatus;
use crate::testing::{agent_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// How many times to yield to the runtime after an action, letting the spawned reactor (and the
/// process actors) make progress before the assertion — the deterministic stand-in for waiting.
const YIELDS: usize = 64;

struct Harness {
    sup: Arc<Supervisor>,
    mailbox: Arc<AgentMailbox>,
    idle: Arc<IdleTracker>,
    clock: MockClock,
    bus: EventBus,
    /// Every byte written to a process's PTY — the observable of "a wake was submitted".
    input: Arc<std::sync::Mutex<Vec<u8>>>,
}

fn harness() -> Harness {
    let bus = EventBus::new(256);
    let clock = MockClock::new();
    let (spawner, input) = FakeSpawner::records_input();
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(clock.clone()),
        Arc::new(FakeTrustRepo::new()),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    Harness {
        sup: Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone())),
        mailbox: Arc::new(AgentMailbox::new(Arc::new(clock.clone()))),
        idle: Arc::new(IdleTracker::new()),
        clock,
        bus,
        input,
    }
}

impl Harness {
    /// Registers a long-lived agent without starting it, so it has no terminal yet and every wake
    /// submission is refused. It is never registered with the idle tracker either, so it stands for
    /// an agent whose provider evidence never arrives: [`IdleTracker::activity`] reports `None` for
    /// it for the whole run.
    fn registered_agent(&self, label: &str) -> ProcessId {
        self.sup.register(agent_registration(PROJECT, label))
    }

    /// Starts a registered agent, returning once it is Running and can accept a wake.
    async fn start(&self, id: ProcessId) {
        self.sup.start(id).expect("start the agent");
        wait_for_running(&mut self.bus.subscribe(), id).await;
    }

    /// A running agent the idle classifier never types.
    async fn unclassified_agent(&self, label: &str) -> ProcessId {
        let id = self.registered_agent(label);
        self.start(id).await;
        id
    }

    fn spawn_reactor(&self) {
        tokio::spawn(
            AgentMailboxReactor::new(
                self.mailbox.clone(),
                self.bus.clone(),
                Arc::downgrade(&self.sup),
                self.idle.clone(),
                Arc::new(self.clock.clone()),
            )
            .run(),
        );
    }

    /// Queues one direct message to `recipient`, returning the id text a wake envelope naming it
    /// must carry — so an assertion reads what reached the agent, not mailbox bookkeeping.
    fn queue_message(&self, recipient: ProcessId) -> String {
        self.mailbox
            .enqueue(
                PROJECT,
                ProcessId::next(),
                recipient,
                AgentMessageKind::Direct,
                "review the parser".to_owned(),
                None,
            )
            .expect("queue a message")
            .message
            .id
            .to_string()
    }

    /// Queues one direct message in `project` and records it, exactly as a facade send does, so a
    /// later transition has an entry to move.
    fn queue_recorded(
        &self,
        project: ProjectId,
        recipient: ProcessId,
        body: &str,
    ) -> AgentMessageId {
        let delivery = self
            .mailbox
            .enqueue(
                project,
                ProcessId::next(),
                recipient,
                AgentMessageKind::Direct,
                body.to_owned(),
                None,
            )
            .expect("queue a message");
        self.mailbox.record(&delivery);
        delivery.message.id
    }

    fn recorded_outcome(
        &self,
        project: ProjectId,
        id: AgentMessageId,
    ) -> Option<AgentMessageOutcome> {
        self.mailbox
            .transcript(project)
            .into_iter()
            .find(|record| record.delivery.message.id == id)
            .map(|record| record.delivery.outcome)
    }

    fn submitted_input(&self) -> String {
        String::from_utf8_lossy(&lock(&self.input)).into_owned()
    }

    fn outcome(&self, recipient: ProcessId) -> AgentMessageOutcome {
        self.mailbox
            .list(recipient)
            .first()
            .expect("the message is still pending")
            .outcome
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

/// Yields to the runtime repeatedly so the spawned reactor (and the process actors) can run.
async fn settle() {
    for _ in 0..YIELDS {
        tokio::task::yield_now().await;
    }
}

/// Advances the clock a backstop's worth at a time and yields until `pred` holds, or fails after a
/// bounded budget — for the backstop path, which only moves when time does.
async fn advance_until<F: Fn() -> bool>(clock: &MockClock, pred: F) {
    for _ in 0..400 {
        clock.advance(MAX_WAKE_WAIT);
        settle().await;
        if pred() {
            return;
        }
    }
    panic!("condition not met within the budget");
}

/// Yields until `pred` holds without advancing time, or fails after a bounded budget — for the
/// idle-event path, where advancing the clock could trip the backstop and mask the result.
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
async fn an_agent_that_is_never_classified_is_woken_once_its_wake_has_waited_out_the_backstop() {
    let h = harness();
    let recipient = h.unclassified_agent("worker").await;
    let message_id = h.queue_message(recipient);
    h.spawn_reactor();
    settle().await;

    advance_until(&h.clock, || h.submitted_input().contains(&message_id)).await;

    assert_eq!(
        h.outcome(recipient),
        AgentMessageOutcome::WakeSubmitted,
        "the backstop wake is reflected in the message's delivery state"
    );
}

#[tokio::test]
async fn a_wake_is_not_delivered_before_its_backstop_elapses() {
    let h = harness();
    let recipient = h.unclassified_agent("worker").await;
    let message_id = h.queue_message(recipient);
    h.spawn_reactor();
    settle().await;

    // A second short of the bound, with the reactor given every chance to sweep: an unclassified
    // agent is left alone until the wait is genuinely spent.
    h.clock.advance(MAX_WAKE_WAIT - Duration::from_secs(1));
    settle().await;

    assert!(
        !h.submitted_input().contains(&message_id),
        "an unclassified agent is not woken before its wake has waited out the bound"
    );
    assert_eq!(h.outcome(recipient), AgentMessageOutcome::Queued);
}

#[tokio::test]
async fn an_agent_that_reaches_idle_is_woken_without_waiting_for_the_backstop() {
    let h = harness();
    let recipient = h.unclassified_agent("worker").await;
    let message_id = h.queue_message(recipient);
    h.spawn_reactor();
    settle().await;

    // No time passes at all here: the idle transition alone delivers the wake.
    h.bus.publish(DomainEvent::AgentActivityChanged {
        id: recipient,
        state: AgentActivity::Idle,
    });
    settle_until(|| h.submitted_input().contains(&message_id)).await;

    assert_eq!(h.outcome(recipient), AgentMessageOutcome::WakeSubmitted);
}

#[tokio::test]
async fn the_backstop_delivers_one_wake_however_long_the_agent_stays_unclassified() {
    let h = harness();
    let recipient = h.unclassified_agent("worker").await;
    let message_id = h.queue_message(recipient);
    h.spawn_reactor();
    settle().await;

    advance_until(&h.clock, || h.submitted_input().contains(&message_id)).await;
    let after_first_wake = h.submitted_input();

    // The agent still never classifies, and ten more backstops' worth of time passes: the wake was
    // one delivery, not the first beat of a recurring poke.
    for _ in 0..10 {
        h.clock.advance(MAX_WAKE_WAIT);
        settle().await;
    }

    assert_eq!(
        h.submitted_input(),
        after_first_wake,
        "the backstop wakes a pending message once, not on a repeating interval"
    );
}

#[tokio::test]
async fn a_backstop_wake_the_agent_refuses_is_delivered_once_it_can_be_accepted() {
    let h = harness();
    let recipient = h.registered_agent("worker");
    let message_id = h.queue_message(recipient);
    h.spawn_reactor();
    settle().await;

    // The wait is spent while the agent has no terminal, so this delivery cannot be accepted.
    h.clock.advance(MAX_WAKE_WAIT);
    settle().await;
    assert!(
        !h.submitted_input().contains(&message_id),
        "an agent that cannot accept the submission is not woken by it"
    );
    assert_eq!(h.outcome(recipient), AgentMessageOutcome::Queued);

    h.start(recipient).await;
    advance_until(&h.clock, || h.submitted_input().contains(&message_id)).await;

    assert_eq!(
        h.outcome(recipient),
        AgentMessageOutcome::WakeSubmitted,
        "a delivery the agent refused left the wake owed rather than spending it"
    );
}

#[tokio::test]
async fn removing_a_project_forgets_its_transcript() {
    let h = harness();
    let kept = ProjectId::from_raw(PROJECT.get() + 1);
    let recipient = h.registered_agent("worker");
    h.queue_recorded(PROJECT, recipient, "in the removed project");
    h.queue_recorded(kept, recipient, "in the kept project");
    h.spawn_reactor();
    settle().await;

    h.bus.publish(DomainEvent::ProjectRemoved { id: PROJECT });
    settle_until(|| h.mailbox.transcript(PROJECT).is_empty()).await;

    assert_eq!(
        h.mailbox.transcript(kept).len(),
        1,
        "only the removed project's history is forgotten",
    );
}

#[tokio::test]
async fn a_wake_the_reactor_delivers_moves_the_record_and_announces_it() {
    let h = harness();
    let recipient = h.unclassified_agent("worker").await;
    let message = h.queue_recorded(PROJECT, recipient, "review the parser");
    h.spawn_reactor();
    settle().await;
    // The send that recorded this exchange found the recipient unclassified, so the record starts
    // out merely queued and only the reactor's later delivery can move it.
    assert_eq!(
        h.recorded_outcome(PROJECT, message),
        Some(AgentMessageOutcome::Queued),
    );
    let mut rx = h.bus.subscribe();

    h.bus.publish(DomainEvent::AgentActivityChanged {
        id: recipient,
        state: AgentActivity::Idle,
    });
    settle_until(|| h.submitted_input().contains(&message.to_string())).await;

    assert_eq!(
        h.recorded_outcome(PROJECT, message),
        Some(AgentMessageOutcome::WakeSubmitted),
        "the transcript follows the wake the reactor delivered, not only the one a send delivers",
    );
    assert!(
        crate::testing::drain(&mut rx).iter().any(|event| matches!(
            event,
            DomainEvent::AgentMessageChanged { project, id }
                if *project == PROJECT && *id == message
        )),
        "the reactor announces the transition it caused",
    );
}
