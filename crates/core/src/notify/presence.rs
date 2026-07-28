//! Where the user is: whether the app's window is on screen, and which process it is showing.
//!
//! An observation the shell makes and pushes in, not state the core derives — the core has no way
//! to see a window. [`PresenceCell`] caches the latest observation for the reactor to read while
//! routing an alert. It is deliberately in memory: it is rewritten on every process selection, so a
//! durable write on that path would put a store round-trip in front of a click.

use std::sync::Mutex;

use serde::Deserialize;

use crate::ids::ProcessId;
use crate::sync::lock;

/// The shell's latest report of where the user is. Defaults to away from the app and looking at
/// nothing, so a surface that never reports presence — a headless MCP or HTTP caller, a test —
/// routes to the desktop rather than to a toast nothing would render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Presence {
    /// Whether Soloist's window currently has focus.
    pub focused: bool,
    /// The process the window is showing, or `None` when it shows no process.
    ///
    /// Deliberately not cleaned up when that process goes away: a
    /// [`ProcessId`] is never reused, so a stale one cannot match a live process and is inert
    /// until the shell reports a new selection. Reaping it here would couple this to the process
    /// registry for no behavioural gain.
    pub viewing: Option<ProcessId>,
}

/// Holds the latest [`Presence`] for the reactor to read.
///
/// The lock covers a two-field `Copy` swap with a single writer (the shell) and a single reader
/// (the reactor), so it is never contended and holds nothing across an await. This is a cached
/// adapter observation rather than domain state, which is why it is a lock and not an actor.
#[derive(Debug, Default)]
pub struct PresenceCell {
    presence: Mutex<Presence>,
}

impl PresenceCell {
    /// A cell reporting the default presence until the shell says otherwise.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records where the user now is, reporting whether it differs from the last observation.
    /// Last write wins: an observation is only ever the newest truth, so an older one has nothing
    /// to merge. The caller announces a change on the bus, so a report that moved nobody must say
    /// so rather than waking every surface to re-read an identical presence.
    pub fn set(&self, presence: Presence) -> bool {
        let mut current = lock(&self.presence);
        let changed = *current != presence;
        *current = presence;
        changed
    }

    /// Where the user was when the shell last reported.
    pub fn get(&self) -> Presence {
        *lock(&self.presence)
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
