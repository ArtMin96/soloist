//! Behavioural tests for the notification port: what the headless default reports about its
//! ability to deliver, and that a sound hint survives the trip through the port to an adapter.

use crate::testing::RecordingNotifier;

use super::{NoopNotifier, Notification, Notifier, NotifierStatus};

fn crash_toast(sound: Option<&str>) -> Notification {
    Notification {
        title: "Web crashed".into(),
        body: "The process exited unexpectedly.".into(),
        sound: sound.map(Into::into),
    }
}

fn a_listening_backend() -> NotifierStatus {
    NotifierStatus::Available {
        server: "test-backend".into(),
        version: "1".into(),
        capabilities: vec!["body".into(), "sound".into()],
    }
}

#[test]
fn noop_notifier_reports_unavailable() {
    assert_eq!(NoopNotifier.status(), NotifierStatus::Unavailable);
}

#[test]
fn notification_carries_an_optional_sound() {
    let notifier = RecordingNotifier::new();

    notifier.notify(crash_toast(None));
    notifier.notify(crash_toast(Some("message-new-instant")));

    let sounds: Vec<Option<String>> = notifier.shown().into_iter().map(|n| n.sound).collect();
    assert_eq!(sounds, vec![None, Some("message-new-instant".to_owned())]);
}

#[test]
fn recording_notifier_reports_the_status_it_was_built_with() {
    assert_eq!(
        RecordingNotifier::new().status(),
        NotifierStatus::Unavailable
    );
    assert_eq!(
        RecordingNotifier::with_status(a_listening_backend()).status(),
        a_listening_backend()
    );
}
