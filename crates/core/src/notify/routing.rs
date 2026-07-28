//! Where an alert goes: the desktop, an in-app toast, or nowhere.
//!
//! One pure function over values, so the rule has exactly one home. Every surface that renders an
//! alert — the desktop notifier, the in-app toast, the unread markers — reads a decision already
//! made here rather than re-deciding from focus and settings, which is what keeps them thin
//! renderers instead of second implementations of this table.

use crate::attention::AttentionKind;
use crate::ids::ProcessId;
use crate::settings::NotificationLevel;

use super::presence::Presence;

/// What should happen to an alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Nothing at all: no desktop notification, no toast, and no unread mark.
    Suppressed,
    /// An in-app toast, for a user who is already looking at Soloist.
    Toast,
    /// A desktop notification, for a user whose attention is elsewhere.
    Native,
}

/// How an alert about `process` should reach the user.
///
/// The level decides whether the signal survives at all; presence then decides where it goes. An
/// alert is suppressed outright when the user is watching the very process that raised it in a
/// window that is on screen — they have seen it happen, so repeating it is noise.
///
/// That suppression requires focus as well as selection. Presence goes stale by design: the shell
/// pushes `focused: false` when the window hides but leaves the selection alone, so a hidden window
/// still reports the process it last showed. Suppressing on selection alone would lose the alert
/// entirely — no notification to see, no toast on a hidden window, and no unread mark waiting —
/// exactly when the user is least able to notice it for themselves.
///
/// `process` is a parameter rather than a field of [`Presence`] because presence describes the
/// user, not the alert: the same observation routes many alerts, each about a different process.
pub fn route(
    process: ProcessId,
    kind: AttentionKind,
    presence: Presence,
    level: NotificationLevel,
) -> Delivery {
    if !level.admits(kind.severity()) {
        return Delivery::Suppressed;
    }
    if !presence.focused {
        return Delivery::Native;
    }
    if presence.viewing == Some(process) {
        return Delivery::Suppressed;
    }
    Delivery::Toast
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
