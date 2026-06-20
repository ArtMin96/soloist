//! Behavioural tests for [`NotificationReactor`]: it composes the right toast for the
//! attention-worthy events, resolves the process label, and honours the global on/off. They
//! drive a real [`Supervisor`] over fakes (for the label read model) and publish events on the
//! bus directly, so the reactor's own logic is tested without the crash machinery (covered in
//! the restart policy's tests).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::CorePorts;
use crate::process::ProcStatus;
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock, RecordingNotifier};

use super::Notification;
use super::NotificationReactor;

const PROJECT: ProjectId = ProjectId::from_raw(1);
const ROOT: &str = "/project";

struct Setup {
    sup: Arc<Supervisor>,
    bus: EventBus,
    notifier: RecordingNotifier,
    enabled: Arc<AtomicBool>,
}

fn setup() -> Setup {
    let bus = EventBus::new(256);
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(MockClock::new()),
        Arc::new(FakeTrustRepo::new()),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    let sup = Arc::new(Supervisor::new(&ports, bus.clone()));
    Setup {
        sup,
        bus,
        notifier: RecordingNotifier::new(),
        enabled: Arc::new(AtomicBool::new(true)),
    }
}

fn command_spec() -> ProcessSpec {
    ProcessSpec {
        command: "sleep 60".into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

/// Registers a command so the reactor can resolve its label; returns its id.
fn register(s: &Setup, name: &str) -> ProcessId {
    s.sup.register(Registration::command(
        PROJECT,
        Path::new(ROOT),
        name,
        &command_spec(),
    ))
}

/// Spawns the reactor over the spy notifier and the on/off flag.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        NotificationReactor::new(
            Arc::new(s.notifier.clone()),
            s.enabled.clone(),
            &s.bus,
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
}

async fn yield_many() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

/// Waits until at least `n` toasts have been shown, returning them; panics if they never are.
async fn shown_at_least(s: &Setup, n: usize) -> Vec<Notification> {
    for _ in 0..50 {
        yield_many().await;
        let shown = s.notifier.shown();
        if shown.len() >= n {
            return shown;
        }
    }
    panic!(
        "expected at least {n} notification(s), saw {}",
        s.notifier.shown().len()
    );
}

fn crashed(id: ProcessId) -> DomainEvent {
    DomainEvent::ProcessStatusChanged {
        id,
        from: ProcStatus::Running,
        to: ProcStatus::Crashed,
        exit_code: Some(1),
    }
}

#[tokio::test]
async fn a_crash_shows_a_toast_naming_the_process() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    s.bus.publish(crashed(web));

    let shown = shown_at_least(&s, 1).await;
    assert_eq!(shown[0].title, "Web crashed");
}

#[tokio::test]
async fn an_exhausted_auto_restart_shows_a_toast() {
    let s = setup();
    let worker = register(&s, "Worker");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::RestartExhausted { id: worker });

    let shown = shown_at_least(&s, 1).await;
    assert_eq!(shown[0].title, "Worker stopped");
}

#[tokio::test]
async fn notifications_are_silenced_when_disabled() {
    let s = setup();
    let web = register(&s, "Web");
    s.enabled.store(false, Ordering::Relaxed);
    spawn_reactor(&s);

    s.bus.publish(crashed(web));
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "a disabled notifier shows nothing",
    );
}

#[tokio::test]
async fn a_non_attention_event_shows_nothing() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    // A clean stop is not an attention event — only a crash or an exhausted restart is.
    s.bus.publish(DomainEvent::ProcessStatusChanged {
        id: web,
        from: ProcStatus::Stopping,
        to: ProcStatus::Stopped,
        exit_code: Some(0),
    });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "a clean stop warrants no notification",
    );
}
