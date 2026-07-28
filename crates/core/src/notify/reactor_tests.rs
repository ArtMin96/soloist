//! Behavioural tests for [`NotificationReactor`]: it composes the right toast for the
//! attention-worthy events, resolves the process label, and honours the global master switch and
//! the notification level in force for the command. They drive a real [`Supervisor`] over fakes (for the label read
//! model) and publish events on the bus directly, so the reactor's own logic is tested without the
//! crash machinery (covered in the restart policy's tests).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::agents::AgentActivity;
use crate::composition::CorePorts;
use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::process::ProcStatus;
use crate::settings::{NotificationLevel, ProjectSettings, Settings, SettingsStore};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{
    drain, next_matching, FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTrustRepo, MockClock,
    RecordingNotifier,
};

use crate::notify::{AttentionRegistry, Notification, NotificationReactor, Presence, PresenceCell};

const PROJECT: ProjectId = ProjectId::from_raw(1);
const OTHER: ProjectId = ProjectId::from_raw(2);
const ROOT: &str = "/project";
/// How long a test waits for a toast it expects before calling it never shown. Generous — it
/// bounds a failure, and is never reached on the passing path.
const WAIT_FOR_TOAST: Duration = Duration::from_secs(10);

struct Setup {
    sup: Arc<Supervisor>,
    bus: EventBus,
    notifier: RecordingNotifier,
    global: Arc<SettingsStore<(), Settings>>,
    projects: Arc<SettingsStore<ProjectId, ProjectSettings>>,
    presence: Arc<PresenceCell>,
    attention: Arc<AttentionRegistry>,
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
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
    Setup {
        sup,
        bus,
        notifier: RecordingNotifier::new(),
        global: Arc::new(SettingsStore::new(Arc::new(FakeSettingsRepo::new()))),
        projects: Arc::new(SettingsStore::new(Arc::new(FakeSettingsRepo::new()))),
        presence: Arc::new(PresenceCell::new()),
        attention: Arc::new(AttentionRegistry::new()),
    }
}

fn command_spec(auto_restart: bool) -> ProcessSpec {
    ProcessSpec {
        command: "sleep 60".into(),
        working_dir: None,
        auto_start: false,
        auto_restart,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

/// Registers a command under `project` so the reactor can resolve its label; returns its id.
fn register_in(s: &Setup, project: ProjectId, name: &str) -> ProcessId {
    s.sup.register(Registration::command(
        project,
        Path::new(ROOT),
        name,
        &command_spec(false),
    ))
}

/// Registers a command under the default project.
fn register(s: &Setup, name: &str) -> ProcessId {
    register_in(s, PROJECT, name)
}

/// Registers a command the crash policy will relaunch — the self-healing case whose per-attempt
/// crashes are silent.
fn register_auto_restarting(s: &Setup, name: &str) -> ProcessId {
    s.sup.register(Registration::command(
        PROJECT,
        Path::new(ROOT),
        name,
        &command_spec(true),
    ))
}

/// Spawns the reactor over the spy notifier, the settings stores, and the presence and unread
/// state it routes by.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        NotificationReactor::new(
            Arc::new(s.notifier.clone()),
            s.global.clone(),
            s.projects.clone(),
            s.presence.clone(),
            s.attention.clone(),
            &s.bus,
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
}

/// Reports the user as looking at Soloist, showing `viewing`.
fn user_is_here(s: &Setup, viewing: Option<ProcessId>) {
    s.presence.set(Presence {
        focused: true,
        viewing,
    });
}

async fn yield_many() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

/// Awaits `n` toasts and returns them, giving up rather than waiting forever, so a regression that
/// silences a toast that should fire reddens the test instead of hanging the suite. Every wait in
/// this file goes through here: an unbounded one turns a suppressed notification into a permanent
/// block, which reads as a slow run rather than a failure.
async fn expect_shown(s: &Setup, n: usize) -> Vec<Notification> {
    tokio::time::timeout(WAIT_FOR_TOAST, s.notifier.wait_until_shown(n))
        .await
        .expect("the expected toasts were never shown")
}

/// Awaits the toast the reactor should have put on the bus and returns it, giving up rather than
/// waiting forever. Bounded for the same reason [`expect_shown`] is: a regression that stops the
/// toast being emitted must redden this test, not block the suite in a way that reads as slowness.
async fn expect_toast(events: &mut broadcast::Receiver<DomainEvent>) -> DomainEvent {
    tokio::time::timeout(
        WAIT_FOR_TOAST,
        next_matching(events, |event| {
            matches!(event, DomainEvent::NotificationRaised { .. })
        }),
    )
    .await
    .expect("the expected toast never reached the bus")
}

/// Waits for the unread total to reach `expected`, giving up rather than waiting forever. The
/// reactor records unread *after* it delivers, so waiting on a toast does not mean the unread
/// state has caught up yet — a test that read the snapshot straight after would race it.
async fn expect_unread(s: &Setup, expected: usize) {
    let settled = tokio::time::timeout(WAIT_FOR_TOAST, async {
        while s.attention.snapshot().total != expected {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(
        settled.is_ok(),
        "the unread total never reached {expected}; it was {}",
        s.attention.snapshot().total,
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

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Web crashed");
}

#[tokio::test]
async fn an_exhausted_auto_restart_shows_a_toast() {
    let s = setup();
    let worker = register(&s, "Worker");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::RestartExhausted { id: worker });

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Worker stopped");
}

#[tokio::test]
async fn crash_of_an_auto_restart_command_is_not_notified() {
    let s = setup();
    let worker = register_auto_restarting(&s, "Worker");
    spawn_reactor(&s);

    // A command that heals itself retries silently, so a crash loop cannot raise one toast per
    // attempt — only giving up is worth the user's attention.
    s.bus.publish(crashed(worker));
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "a crash the restart policy will retry warrants no notification",
    );
}

#[tokio::test]
async fn restart_exhausted_notifies_once() {
    let s = setup();
    let worker = register_auto_restarting(&s, "Worker");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::RestartExhausted { id: worker });
    expect_shown(&s, 1).await;
    yield_many().await;

    // Silencing the retries must leave the one alert that says the retries are over.
    let shown = s.notifier.shown();
    assert_eq!(shown.len(), 1, "giving up toasts exactly once");
    assert_eq!(shown[0].title, "Worker stopped");
}

#[tokio::test]
async fn crash_without_auto_restart_still_notifies() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    // Nothing will relaunch this command, so its crash is the user's only signal — the retry
    // gate reads the command's policy, never the state it happens to be in.
    s.bus.publish(crashed(web));
    expect_shown(&s, 1).await;
    yield_many().await;

    let shown = s.notifier.shown();
    assert_eq!(
        shown.len(),
        1,
        "a crash nothing will retry toasts exactly once"
    );
    assert_eq!(shown[0].title, "Web crashed");
}

#[tokio::test]
async fn an_agent_awaiting_permission_shows_a_toast() {
    let s = setup();
    let agent = register(&s, "Claude");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::AgentActivityChanged {
        id: agent,
        state: AgentActivity::Permission,
    });

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Claude needs your input");
}

#[tokio::test]
async fn an_agent_error_shows_a_toast() {
    let s = setup();
    let agent = register(&s, "Gemini");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::AgentActivityChanged {
        id: agent,
        state: AgentActivity::Error,
    });

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Gemini hit an error");
}

#[tokio::test]
async fn a_terminal_bell_shows_a_toast() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Web rang the bell");
}

/// A notification a script raised for itself, with the words it chose.
fn script_notification(id: ProcessId, title: Option<&str>, body: &str) -> DomainEvent {
    DomainEvent::TerminalNotification {
        id,
        title: title.map(str::to_owned),
        body: body.to_owned(),
    }
}

#[tokio::test]
async fn terminal_notification_uses_the_scripts_own_text() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    s.bus
        .publish(script_notification(web, Some("Build"), "done"));

    // The whole point of the feature: the script said what it wanted said, so nothing here may
    // replace it with a sentence Soloist wrote about a process ringing a bell.
    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Build");
    assert_eq!(shown[0].body, "done");
}

#[tokio::test]
async fn a_titleless_notification_falls_back_to_the_process_label() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    // OSC 9 carries a message and no title, so the process's own name is the heading.
    s.bus.publish(script_notification(web, None, "done"));

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Web");
    assert_eq!(shown[0].body, "done");
}

#[tokio::test]
async fn level_important_silences_a_terminal_notification() {
    let s = setup();
    let web = register(&s, "Web");
    s.projects
        .update(&PROJECT, |p| {
            p.notification_level = NotificationLevel::Important
        })
        .unwrap();
    spawn_reactor(&s);

    s.bus
        .publish(script_notification(web, Some("Build"), "done"));
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "a script's own notification is terminal-class, so level Important drops it",
    );
}

#[tokio::test]
async fn a_busy_agent_shows_nothing() {
    let s = setup();
    let agent = register(&s, "Claude");
    spawn_reactor(&s);

    // Working/Idle/Thinking are not attention states — only Permission and Error toast.
    s.bus.publish(DomainEvent::AgentActivityChanged {
        id: agent,
        state: AgentActivity::Working,
    });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "an agent working warrants no notification",
    );
}

#[tokio::test]
async fn level_none_silences_a_crash() {
    let s = setup();
    let web = register(&s, "Web");
    s.projects
        .update(&PROJECT, |p| p.notification_level = NotificationLevel::None)
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(crashed(web));
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "at level None, a crash raises no toast",
    );
}

#[tokio::test]
async fn crash_alerts_are_scoped_to_the_crashing_process_project() {
    let s = setup();
    // Silent for PROJECT, default for OTHER — a crash in each must respect its own project.
    let hushed = register_in(&s, PROJECT, "Hushed");
    let loud = register_in(&s, OTHER, "Loud");
    s.projects
        .update(&PROJECT, |p| p.notification_level = NotificationLevel::None)
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(crashed(hushed));
    s.bus.publish(crashed(loud));

    let shown = expect_shown(&s, 1).await;
    assert_eq!(
        shown.len(),
        1,
        "only the other project's crash toasts; the hushed project's is suppressed",
    );
    assert_eq!(shown[0].title, "Loud crashed");
}

#[tokio::test]
async fn level_important_silences_a_bell() {
    let s = setup();
    let web = register(&s, "Web");
    s.projects
        .update(&PROJECT, |p| {
            p.notification_level = NotificationLevel::Important
        })
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "at level Important a bell is terminal-class, so it raises no toast",
    );
}

#[tokio::test]
async fn level_important_keeps_an_agent_asking_for_attention() {
    let s = setup();
    let agent = register(&s, "Claude");
    s.projects
        .update(&PROJECT, |p| {
            p.notification_level = NotificationLevel::Important
        })
        .unwrap();
    spawn_reactor(&s);

    // A blocked agent is a state a human must clear before anything proceeds, so it ranks with the
    // crashes rather than with the bells and survives everything but silence.
    s.bus.publish(DomainEvent::AgentActivityChanged {
        id: agent,
        state: AgentActivity::Permission,
    });

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Claude needs your input");
}

#[tokio::test]
async fn level_none_silences_an_agent_asking_for_attention() {
    let s = setup();
    let agent = register(&s, "Claude");
    s.projects
        .update(&PROJECT, |p| p.notification_level = NotificationLevel::None)
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::AgentActivityChanged {
        id: agent,
        state: AgentActivity::Permission,
    });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "level None is the one setting that silences an agent waiting on the user",
    );
}

#[tokio::test]
async fn a_per_command_override_quietens_one_command() {
    let s = setup();
    let web = register(&s, "Web");
    let api = register(&s, "Api");
    // The project admits everything, but "Web" is individually held to the important ones.
    s.projects
        .update(&PROJECT, |p| {
            p.command_notification_levels
                .insert("Web".into(), NotificationLevel::Important);
        })
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });
    s.bus.publish(DomainEvent::TerminalBell { id: api });

    let shown = expect_shown(&s, 1).await;
    assert_eq!(
        shown.len(),
        1,
        "the quietened command rings no toast; the other still does",
    );
    assert_eq!(shown[0].title, "Api rang the bell");
}

#[tokio::test]
async fn command_override_tightens_but_cannot_loosen() {
    let s = setup();
    let web = register(&s, "Web");
    s.projects
        .update(&PROJECT, |p| {
            p.notification_level = NotificationLevel::Important;
            p.command_notification_levels
                .insert("Web".into(), NotificationLevel::All);
        })
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "the project and the command combine to the more restrictive of the two, so a looser \
         command setting cannot re-admit what the project silenced",
    );
}

#[tokio::test]
async fn the_global_master_switch_silences_everything() {
    let s = setup();
    let web = register(&s, "Web");
    s.global
        .update(&(), |g| g.notifications.enabled = false)
        .unwrap();
    spawn_reactor(&s);

    // Off globally: neither a crash nor a bell fires, whatever the project level admits.
    s.bus.publish(crashed(web));
    s.bus.publish(DomainEvent::TerminalBell { id: web });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "the global master switch off silences every toast",
    );
}

#[tokio::test]
async fn unfocused_calls_the_notifier_not_a_toast() {
    let s = setup();
    let web = register(&s, "Web");
    let mut events = s.bus.subscribe();
    spawn_reactor(&s);

    // No presence was ever reported, which is what every headless caller looks like. Getting this
    // default wrong would route their alerts to a toast surface that does not exist.
    s.bus.publish(crashed(web));

    let shown = expect_shown(&s, 1).await;
    assert_eq!(shown[0].title, "Web crashed");
    assert!(!drain(&mut events)
        .iter()
        .any(|event| matches!(event, DomainEvent::NotificationRaised { .. })));
}

#[tokio::test]
async fn focused_elsewhere_emits_a_toast_not_a_native_notification() {
    let s = setup();
    let web = register(&s, "Web");
    let api = register(&s, "Api");
    let mut events = s.bus.subscribe();
    user_is_here(&s, Some(api));
    spawn_reactor(&s);

    s.bus.publish(crashed(web));

    match expect_toast(&mut events).await {
        DomainEvent::NotificationRaised {
            process,
            title,
            body,
            ..
        } => {
            assert_eq!(process, web);
            // The toast carries the composed text, so the desktop and the in-app surface can
            // never word the same event differently.
            assert_eq!(title, "Web crashed");
            assert_eq!(body, "The process exited unexpectedly.");
        }
        other => panic!("unexpected event: {other:?}"),
    }
    yield_many().await;
    assert!(
        s.notifier.shown().is_empty(),
        "a user looking at the app gets a toast, never a desktop notification",
    );
}

#[tokio::test]
async fn a_delivered_alert_is_marked_unread() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    s.bus.publish(crashed(web));
    expect_unread(&s, 1).await;

    assert_eq!(s.attention.snapshot().processes[0].process, web);
}

#[tokio::test]
async fn viewing_the_process_suppresses_everything() {
    let s = setup();
    let web = register(&s, "Web");
    let mut events = s.bus.subscribe();
    user_is_here(&s, Some(web));
    spawn_reactor(&s);

    s.bus.publish(crashed(web));
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "the user watched it happen, so no desktop notification fires",
    );
    assert!(
        !drain(&mut events)
            .iter()
            .any(|event| matches!(event, DomainEvent::NotificationRaised { .. })),
        "nor a toast",
    );
    assert_eq!(
        s.attention.snapshot().total,
        0,
        "nor an unread mark: suppression means nothing at all, not a silent alert left to dismiss",
    );
}

#[tokio::test]
async fn a_suppressed_alert_leaves_earlier_unread_alone() {
    let s = setup();
    let web = register(&s, "Web");
    let api = register(&s, "Api");
    spawn_reactor(&s);

    s.bus.publish(crashed(api));
    expect_unread(&s, 1).await;
    user_is_here(&s, Some(web));
    s.bus.publish(crashed(web));
    yield_many().await;

    // Suppressing an alert is not clearing the unread state; only seeing a process clears it.
    assert_eq!(s.attention.snapshot().total, 1);
}

#[tokio::test]
async fn attention_does_not_leak_across_a_start_stop_cycle() {
    let s = setup();
    spawn_reactor(&s);

    // Each process crashes, is marked unread, then leaves the registry. Nothing can visit a
    // process that no longer exists, so a stranded entry would sit in the count forever.
    for n in 0..5 {
        let id = register(&s, &format!("Worker{n}"));
        s.bus.publish(crashed(id));
        expect_unread(&s, 1).await;
        s.bus.publish(DomainEvent::ProcessRemoved { id });
        expect_unread(&s, 0).await;
    }

    assert_eq!(s.attention.snapshot().total, 0);
}

#[tokio::test]
async fn a_removed_process_with_nothing_unread_announces_nothing() {
    let s = setup();
    let web = register(&s, "Web");
    let mut events = s.bus.subscribe();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::ProcessRemoved { id: web });
    yield_many().await;

    assert!(
        !drain(&mut events)
            .iter()
            .any(|event| matches!(event, DomainEvent::AttentionChanged)),
        "every surface re-reads the snapshot on this event, so it must mark a real change",
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
