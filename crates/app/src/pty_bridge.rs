//! Adapter-side PTY streaming lifecycle.
//!
//! The dashboard shows one terminal at a time, so at most one live forwarder runs: a
//! task draining a process's core output broadcast into a Tauri [`Channel`]. This holds
//! that single forwarder's handle, so attaching a new process — or closing the pane —
//! deterministically cancels the previous one and no streaming task leaks.
//!
//! Each install is identified by a monotonically increasing token, and a detach names the
//! attachment it targets. Async commands execute out of invoke order, so a detach issued
//! for an old attachment can arrive after a newer attach; matching by token makes that
//! stale detach a no-op instead of killing the stream the pane is showing. It is
//! transport state the webview owns, not domain state.
//!
//! [`Channel`]: tauri::ipc::Channel

use std::sync::{Mutex, PoisonError};

use tauri::async_runtime::JoinHandle;

/// Holds the single active PTY-forwarding task, cancelling the previous one on swap.
#[derive(Default)]
pub struct PtyBridge {
    slot: Mutex<Slot>,
}

#[derive(Default)]
struct Slot {
    token: u64,
    forwarder: Option<JoinHandle<()>>,
}

impl PtyBridge {
    /// Installs a new forwarder, aborting any previous one, and returns the token that
    /// identifies this attachment for a later [`clear`](Self::clear).
    pub fn install(&self, handle: JoinHandle<()>) -> u64 {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        slot.token += 1;
        if let Some(previous) = slot.forwarder.replace(handle) {
            previous.abort();
        }
        slot.token
    }

    /// Cancels the forwarder installed under `token` — the pane closed or switched away.
    /// A stale token (a newer attachment has since installed) is a no-op.
    pub fn clear(&self, token: u64) {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if token != slot.token {
            return;
        }
        if let Some(previous) = slot.forwarder.take() {
            previous.abort();
        }
    }
}

#[cfg(test)]
#[path = "pty_bridge_tests.rs"]
mod tests;
