//! Notifications settings (global Notifications tab): the master on/off for every desktop toast,
//! and the sound an alert asks for.
//!
//! This is the top-level gate the [notification reactor](crate::notify) consults before composing a
//! toast — off silences notifications everywhere, regardless of the per-project switches (crash/exit
//! and terminal alerts live on the [project settings](super::ProjectSettings), not here). A persisted
//! preference; the surface it gates is the reactor itself.

use serde::{Deserialize, Serialize};

/// The Notifications tab document — the master notifications toggle and the alert sound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    /// Show desktop notifications at all. Off silences every toast; on defers to the per-project
    /// crash/exit and terminal-alert switches.
    pub enabled: bool,
    /// The sound every alert asks for, or `None` to alert silently. Carried onto each composed
    /// [`Notification`](crate::notify::Notification) as its
    /// [`sound`](crate::notify::Notification::sound) hint, so the one preference reaches both
    /// surfaces an alert can land on.
    ///
    /// A free string rather than an enum: the name is resolved by whatever renders the alert (the
    /// desktop's sound theme), so the set of valid names belongs to the machine, not to the domain.
    /// Silence is the default, because a sound the user did not ask for is the kind of alert that
    /// gets notifications turned off entirely.
    pub bell: Option<String>,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            bell: None,
        }
    }
}
