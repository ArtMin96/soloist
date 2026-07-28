//! Presence and unread actions (context C8 → notifications): where the user is, and what they
//! have not looked at yet.
//!
//! These are the local user's own state, so they live on [`Facade`] and never on
//! [`ScopedFacade`](super::ScopedFacade): a session reaching the core over MCP is another agent,
//! not the person at the keyboard, and must not be able to say where that person is looking or
//! dismiss what they have not read.

use super::Facade;
use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::notify::{AttentionSnapshot, Notification, Presence};

/// The test alert's text. Fixed rather than composed from an event: it stands for no real signal,
/// and its whole job is to prove the channel works.
const TEST_NOTIFICATION_TITLE: &str = "Soloist notifications are working";
const TEST_NOTIFICATION_BODY: &str = "This is what an alert from Soloist looks like.";

impl Facade {
    /// Records where the user is: whether the window has focus, and which process it shows.
    ///
    /// A command rather than a setter — looking at a process is what clears its unread mark.
    /// Deciding that here is what keeps a shell to reporting only what it observes.
    ///
    /// **Focus alone clears nothing.** Unread exists precisely because a toast auto-dismisses and
    /// a desktop notification gets swept away, leaving no way to find which process wanted you —
    /// and you have to be at the window to look. Emptying the set on arrival would make the
    /// markers unobservable by construction. The app-icon badge does clear on focus, but that is
    /// one surface choosing what to draw from an unchanged snapshot, not the core forgetting.
    ///
    /// Reporting presence cannot drive a cycle, though surfaces re-read on the announcement: an
    /// announcement needs a clear that actually removed something, and nothing can be removed
    /// twice without the reactor raising it again — which, for the process on screen, it will not
    /// do.
    pub fn set_presence(&self, presence: Presence) {
        self.presence.set(presence);
        // A process is only really seen when the window showing it is on screen, so a selection
        // made in a background window clears nothing.
        let seen = presence.focused
            && presence
                .viewing
                .is_some_and(|process| self.attention.clear(process));
        if seen {
            self.bus.publish(DomainEvent::AttentionChanged);
        }
    }

    /// Everything currently unread: which processes are waiting on the user and how much in total.
    /// The single source both the in-app markers and the app-icon badge render, derived on read so
    /// neither can drift from the other.
    pub fn attention_snapshot(&self) -> AttentionSnapshot {
        self.attention.snapshot()
    }

    /// Dismisses what one process had unread.
    pub fn clear_attention(&self, process: ProcessId) {
        if self.attention.clear(process) {
            self.bus.publish(DomainEvent::AttentionChanged);
        }
    }

    /// Dismisses everything unread — the "clear all" the title-bar count offers.
    pub fn clear_all_attention(&self) {
        if self.attention.clear_all() {
            self.bus.publish(DomainEvent::AttentionChanged);
        }
    }

    /// Shows a sample desktop notification, so a user can tell whether alerts reach them at all.
    ///
    /// Deliberately outside the routing rules: it is a diagnostic for the desktop channel, so it
    /// goes straight to the notifier and marks nothing unread. Routing it would make it show
    /// nothing at all for the focused user who is most likely to press it. Best-effort like every
    /// notification: with no backend listening it is silently dropped, which is itself the answer
    /// the user pressed it for.
    pub fn send_test_notification(&self) {
        self.notifier.notify(Notification {
            title: TEST_NOTIFICATION_TITLE.into(),
            body: TEST_NOTIFICATION_BODY.into(),
            sound: None,
        });
    }
}

#[cfg(test)]
#[path = "attention_tests.rs"]
mod tests;
