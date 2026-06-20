//! A [`Notifier`] fake that records the notifications it was asked to show, so a test can
//! assert the reactor composes the right toast for a crash or an exhausted auto-restart.

use std::sync::{Arc, Mutex};

use crate::notify::{Notification, Notifier};
use crate::sync::lock;

/// A [`Notifier`] that records every notification it was handed, in order.
#[derive(Clone, Default)]
pub struct RecordingNotifier {
    shown: Arc<Mutex<Vec<Notification>>>,
}

impl RecordingNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// The notifications shown so far, in order.
    pub fn shown(&self) -> Vec<Notification> {
        lock(&self.shown).clone()
    }
}

impl Notifier for RecordingNotifier {
    fn notify(&self, notification: Notification) {
        lock(&self.shown).push(notification);
    }
}
