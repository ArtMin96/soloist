//! Adapter-side PTY streaming lifecycle.
//!
//! The dashboard keeps a bounded pool of terminals alive (one per recently-viewed process) so
//! switching between them is instant, so several forwarders run at once: each is a task draining
//! one process's core output broadcast into a Tauri [`Channel`]. This holds those forwarders keyed
//! by an install token, so detaching one pane (or evicting it from the pool) cancels exactly its
//! forwarder and no streaming task leaks.
//!
//! Each install is identified by a monotonically increasing token, and a detach names the
//! attachment it targets. Async commands execute out of invoke order, so a detach issued for an
//! attachment that has already been cleared is a no-op (its token is gone) rather than touching a
//! live one. It is transport state the webview owns, not domain state.
//!
//! [`Channel`]: tauri::ipc::Channel

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use tauri::async_runtime::JoinHandle;

/// Holds the live PTY-forwarding tasks, one per attached terminal, keyed by install token.
#[derive(Default)]
pub struct PtyBridge {
    slots: Mutex<Slots>,
}

#[derive(Default)]
struct Slots {
    next_token: u64,
    forwarders: HashMap<u64, JoinHandle<()>>,
}

impl PtyBridge {
    /// Installs a new forwarder and returns the token that identifies this attachment for a later
    /// [`clear`](Self::clear). Other forwarders keep running — the terminal pool streams several
    /// processes at once.
    pub fn install(&self, handle: JoinHandle<()>) -> u64 {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots.next_token += 1;
        let token = slots.next_token;
        slots.forwarders.insert(token, handle);
        token
    }

    /// Cancels the forwarder installed under `token` — the pane closed, switched away, or was
    /// evicted from the pool. A stale token (already cleared) is a no-op.
    pub fn clear(&self, token: u64) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(forwarder) = slots.forwarders.remove(&token) {
            forwarder.abort();
        }
    }
}

#[cfg(test)]
#[path = "pty_bridge_tests.rs"]
mod tests;
