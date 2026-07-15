//! Behavioural tests for [`NotificationReactor`]: it composes the right toast for the
//! attention-worthy events, resolves the process label, and honours the global master switch and
//! the per-project alert switches. They drive a real [`Supervisor`] over fakes (for the label read
//! model) and publish events on the bus directly, so the reactor's own logic is tested without the
//! crash machinery (covered in the restart policy's tests).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::agents::AgentActivity;
use crate::composition::CorePorts;
use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::process::ProcStatus;
use crate::settings::{ProjectSettings, Settings, SettingsStore};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{
    FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTrustRepo, MockClock, RecordingNotifier,
};

use super::NotificationReactor;

const PROJECT: ProjectId = ProjectId::from_raw(1);
const OTHER: ProjectId = ProjectId::from_raw(2);
const ROOT: &str = "/project";

struct Setup {
    sup: Arc<Supervisor>,
    bus: EventBus,
    notifier: RecordingNotifier,
    global: Arc<SettingsStore<(), Settings>>,
    projects: Arc<SettingsStore<ProjectId, ProjectSettings>>,
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

/// Registers a command under `project` so the reactor can resolve its label; returns its id.
fn register_in(s: &Setup, project: ProjectId, name: &str) -> ProcessId {
    s.sup.register(Registration::command(
        project,
        Path::new(ROOT),
        name,
        &command_spec(),
    ))
}

/// Registers a command under the default project.
fn register(s: &Setup, name: &str) -> ProcessId {
    register_in(s, PROJECT, name)
}

/// Spawns the reactor over the spy notifier and the settings stores.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        NotificationReactor::new(
            Arc::new(s.notifier.clone()),
            s.global.clone(),
            s.projects.clone(),
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

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(shown[0].title, "Web crashed");
}

#[tokio::test]
async fn an_exhausted_auto_restart_shows_a_toast() {
    let s = setup();
    let worker = register(&s, "Worker");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::RestartExhausted { id: worker });

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(shown[0].title, "Worker stopped");
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

    let shown = s.notifier.wait_until_shown(1).await;
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

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(shown[0].title, "Gemini hit an error");
}

#[tokio::test]
async fn a_terminal_bell_shows_a_toast() {
    let s = setup();
    let web = register(&s, "Web");
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(shown[0].title, "Web rang the bell");
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
async fn crash_alerts_off_silences_a_crash() {
    let s = setup();
    let web = register(&s, "Web");
    s.projects
        .update(&PROJECT, |p| p.crash_exit_alerts = false)
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(crashed(web));
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "with crash & exit alerts off, a crash raises no toast",
    );
}

#[tokio::test]
async fn crash_alerts_are_scoped_to_the_crashing_process_project() {
    let s = setup();
    // Off for PROJECT, on (default) for OTHER — a crash in each must respect its own project.
    let hushed = register_in(&s, PROJECT, "Hushed");
    let loud = register_in(&s, OTHER, "Loud");
    s.projects
        .update(&PROJECT, |p| p.crash_exit_alerts = false)
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(crashed(hushed));
    s.bus.publish(crashed(loud));

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(
        shown.len(),
        1,
        "only the other project's crash toasts; the hushed project's is suppressed",
    );
    assert_eq!(shown[0].title, "Loud crashed");
}

#[tokio::test]
async fn terminal_alerts_off_silences_a_bell() {
    let s = setup();
    let web = register(&s, "Web");
    s.projects
        .update(&PROJECT, |p| p.terminal_alerts = false)
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "with terminal alerts off, a bell raises no toast",
    );
}

#[tokio::test]
async fn terminal_alerts_off_silences_an_agent_asking_for_attention() {
    let s = setup();
    let agent = register(&s, "Claude");
    s.projects
        .update(&PROJECT, |p| p.terminal_alerts = false)
        .unwrap();
    spawn_reactor(&s);

    // "Terminal alerts" gates both the bell and an agent asking for attention.
    s.bus.publish(DomainEvent::AgentActivityChanged {
        id: agent,
        state: AgentActivity::Permission,
    });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "with terminal alerts off, an agent permission prompt raises no toast",
    );
}

#[tokio::test]
async fn a_per_command_terminal_override_wins_over_the_project_default() {
    let s = setup();
    let web = register(&s, "Web");
    let api = register(&s, "Api");
    // Project default on, but "Web" is individually silenced.
    s.projects
        .update(&PROJECT, |p| {
            p.command_terminal_alerts.insert("Web".into(), false);
        })
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });
    s.bus.publish(DomainEvent::TerminalBell { id: api });

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(
        shown.len(),
        1,
        "the silenced command rings no toast; the other still does",
    );
    assert_eq!(shown[0].title, "Api rang the bell");
}

#[tokio::test]
async fn a_per_command_terminal_override_can_re_enable_a_silenced_project() {
    let s = setup();
    let web = register(&s, "Web");
    // Project default off, but "Web" is individually re-enabled — the override wins either way.
    s.projects
        .update(&PROJECT, |p| {
            p.terminal_alerts = false;
            p.command_terminal_alerts.insert("Web".into(), true);
        })
        .unwrap();
    spawn_reactor(&s);

    s.bus.publish(DomainEvent::TerminalBell { id: web });

    let shown = s.notifier.wait_until_shown(1).await;
    assert_eq!(shown[0].title, "Web rang the bell");
}

#[tokio::test]
async fn the_global_master_switch_silences_everything() {
    let s = setup();
    let web = register(&s, "Web");
    s.global
        .update(&(), |g| g.notifications.enabled = false)
        .unwrap();
    spawn_reactor(&s);

    // Off globally: neither a crash (crash/exit alerts on per project) nor a bell fires.
    s.bus.publish(crashed(web));
    s.bus.publish(DomainEvent::TerminalBell { id: web });
    yield_many().await;

    assert!(
        s.notifier.shown().is_empty(),
        "the global master switch off silences every toast",
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
