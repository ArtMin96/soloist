//! Adapter-side PTY streaming lifecycle.
//!
//! The dashboard shows one terminal at a time, so at most one live forwarder runs: a
//! task draining a process's core output broadcast into a Tauri [`Channel`]. This holds
//! that single forwarder's handle, so attaching a new process — or closing the pane —
//! deterministically cancels the previous one and no streaming task leaks. It is
//! transport state the webview owns, not domain state.
//!
//! [`Channel`]: tauri::ipc::Channel

use std::sync::{Mutex, PoisonError};

use tauri::async_runtime::JoinHandle;

/// Holds the single active PTY-forwarding task, cancelling the previous one on swap.
#[derive(Default)]
pub struct PtyBridge {
    forwarder: Mutex<Option<JoinHandle<()>>>,
}

impl PtyBridge {
    /// Installs a new forwarder, aborting any previous one.
    pub fn install(&self, handle: JoinHandle<()>) {
        let mut slot = self
            .forwarder
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(previous) = slot.replace(handle) {
            previous.abort();
        }
    }

    /// Cancels the current forwarder, if any — the pane closed or switched away.
    pub fn clear(&self) {
        let mut slot = self
            .forwarder
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(previous) = slot.take() {
            previous.abort();
        }
    }
}
