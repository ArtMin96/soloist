//! Presence and unread behaviour at the façade: which observations clear what, and what reaches
//! the bus. These drive a real [`Facade`] over fakes and assert the snapshot and the event stream,
//! never how the registry was called.

use std::sync::Arc;

use crate::attention::AttentionKind;
use crate::composition::CorePorts;
use crate::events::DomainEvent;
use crate::facade::Facade;
use crate::ids::ProcessId;
use crate::notify::Presence;
use crate::testing::{
    drain, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock, RecordingNotifier,
};

const WEB: ProcessId = ProcessId::from_raw(1);
const API: ProcessId = ProcessId::from_raw(2);

fn facade() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(MockClock::new()),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    )
}

/// A façade whose notifier records what it was asked to show.
fn facade_with_notifier(notifier: &RecordingNotifier) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(MockClock::new()),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .notifier(Arc::new(notifier.clone()))
        .build(),
    )
}

fn away() -> Presence {
    Presence {
        focused: false,
        viewing: None,
    }
}

fn here_looking_at(process: ProcessId) -> Presence {
    Presence {
        focused: true,
        viewing: Some(process),
    }
}

/// Marks `process` unread through the same registry the reactor writes to, so a test can then
/// observe what clears it.
fn raise(facade: &Facade, process: ProcessId) {
    facade.attention.raise(process, AttentionKind::Crashed);
}

#[tokio::test]
async fn a_fresh_facade_has_nothing_unread() {
    assert_eq!(facade().attention_snapshot().total, 0);
}

#[tokio::test]
async fn arriving_at_the_window_does_not_clear_unread() {
    let facade = facade();
    let mut events = facade.subscribe();
    raise(&facade, WEB);
    raise(&facade, API);

    facade.set_presence(Presence {
        focused: true,
        viewing: None,
    });

    // Unread exists so a user can find what wanted them after the toast vanished — and they have
    // to be at the window to look. Clearing on arrival would make the markers unobservable.
    assert_eq!(facade.attention_snapshot().total, 2);
    assert!(!drain(&mut events)
        .iter()
        .any(|event| matches!(event, DomainEvent::AttentionChanged)));
}

#[tokio::test]
async fn a_presence_report_announces_itself_and_is_readable() {
    let facade = facade();
    let mut events = facade.subscribe();

    facade.set_presence(here_looking_at(WEB));

    // The badge draws from where the user is, not only from what is unread, so walking to or from
    // the window has to reach the bus even when nothing unread changed — and be readable when it
    // gets there.
    assert!(drain(&mut events)
        .iter()
        .any(|event| matches!(event, DomainEvent::PresenceChanged)));
    assert_eq!(facade.presence(), here_looking_at(WEB));
}

#[tokio::test]
async fn repeating_a_presence_report_announces_nothing() {
    let facade = facade();
    facade.set_presence(here_looking_at(WEB));
    let mut events = facade.subscribe();

    facade.set_presence(here_looking_at(WEB));

    // A report that moved nobody would otherwise wake every surface to re-read an identical
    // presence.
    assert!(drain(&mut events).is_empty());
}

#[tokio::test]
async fn looking_at_a_process_clears_only_that_process() {
    let facade = facade();
    // Both alerted while the user was away, so both went to the desktop and both are unread.
    raise(&facade, WEB);
    raise(&facade, API);

    facade.set_presence(here_looking_at(WEB));

    let snapshot = facade.attention_snapshot();
    assert_eq!(snapshot.total, 1);
    assert_eq!(snapshot.processes[0].process, API);
}

#[tokio::test]
async fn a_selection_in_a_background_window_clears_nothing() {
    let facade = facade();
    raise(&facade, WEB);

    // The window is not on screen, so nothing has actually been seen.
    facade.set_presence(Presence {
        focused: false,
        viewing: Some(WEB),
    });

    assert_eq!(facade.attention_snapshot().total, 1);
}

#[tokio::test]
async fn losing_focus_clears_nothing() {
    let facade = facade();
    facade.set_presence(here_looking_at(API));
    raise(&facade, WEB);

    facade.set_presence(away());

    assert_eq!(facade.attention_snapshot().total, 1);
}

#[tokio::test]
async fn clearing_one_process_leaves_the_rest() {
    let facade = facade();
    raise(&facade, WEB);
    raise(&facade, API);

    facade.clear_attention(WEB);

    let snapshot = facade.attention_snapshot();
    assert_eq!(snapshot.total, 1);
    assert_eq!(snapshot.processes[0].process, API);
}

#[tokio::test]
async fn clearing_all_empties_the_snapshot() {
    let facade = facade();
    raise(&facade, WEB);
    raise(&facade, API);

    facade.clear_all_attention();

    assert_eq!(facade.attention_snapshot().total, 0);
}

#[tokio::test]
async fn a_clear_that_changes_nothing_announces_nothing() {
    let facade = facade();
    let mut events = facade.subscribe();

    facade.clear_attention(WEB);
    facade.clear_all_attention();
    facade.set_presence(here_looking_at(WEB));

    // Every surface re-reads the snapshot on this event, so announcing a change that did not
    // happen makes each of them do that work for an unchanged answer.
    assert!(!drain(&mut events)
        .iter()
        .any(|event| matches!(event, DomainEvent::AttentionChanged)));
}

#[tokio::test]
async fn a_test_notification_reaches_the_desktop_and_marks_nothing_unread() {
    let notifier = RecordingNotifier::new();
    let facade = facade_with_notifier(&notifier);

    // Pressed by a user who is by definition looking at the window: routing it would suppress the
    // one alert whose whole purpose is to be seen.
    facade.set_presence(here_looking_at(WEB));
    facade.send_test_notification();

    assert_eq!(notifier.shown().len(), 1);
    assert_eq!(facade.attention_snapshot().total, 0);
}
