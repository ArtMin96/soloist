//! Notifications settings (global Notifications tab): the master on/off for every desktop toast.
//!
//! This is the top-level gate the [notification reactor](crate::notify) consults before composing a
//! toast — off silences notifications everywhere, regardless of the per-project switches (crash/exit
//! and terminal alerts live on the [project settings](super::ProjectSettings), not here). A persisted
//! preference; the surface it gates is the reactor itself.

use serde::{Deserialize, Serialize};

/// The Notifications tab document — the master notifications toggle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    /// Show desktop notifications at all. Off silences every toast; on defers to the per-project
    /// crash/exit and terminal-alert switches.
    pub enabled: bool,
}

impl Default for Notifications {
    fn default() -> Self {
        Self { enabled: true }
    }
}
