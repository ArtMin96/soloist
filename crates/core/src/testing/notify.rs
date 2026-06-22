//! A [`Notifier`] fake that records the notifications it was asked to show, so a test can
//! assert the reactor composes the right toast for a crash or an exhausted auto-restart.

use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use crate::notify::{Notification, Notifier};
use crate::sync::lock;

/// A [`Notifier`] that records every notification it was handed, in order.
#[derive(Clone, Default)]
pub struct RecordingNotifier {
    shown: Arc<Mutex<Vec<Notification>>>,
    recorded: Arc<Notify>,
}

impl RecordingNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// The notifications shown so far, in order.
    pub fn shown(&self) -> Vec<Notification> {
        lock(&self.shown).clone()
    }

    /// Awaits until at least `n` notifications have been recorded, returning them in order. A
    /// deterministic signal to await instead of polling a yield budget: it suspends until the
    /// reactor actually shows a toast, so the runtime is free to schedule the reactor whenever
    /// it is ready.
    pub async fn wait_until_shown(&self, n: usize) -> Vec<Notification> {
        loop {
            // Arm the wait before reading, so a toast recorded in the gap is never missed.
            let recorded = self.recorded.notified();
            {
                let shown = lock(&self.shown);
                if shown.len() >= n {
                    return shown.clone();
                }
            }
            recorded.await;
        }
    }
}

impl Notifier for RecordingNotifier {
    fn notify(&self, notification: Notification) {
        lock(&self.shown).push(notification);
        self.recorded.notify_one();
    }
}
