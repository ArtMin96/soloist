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
//! A detach is the webview's responsibility, so a lost token — a full page reload (dev HMR) or a
//! web-process crash, where the JS holding the tokens is gone — would otherwise strand its
//! forwarder forever. A ceiling ([`MAX_FORWARDERS`]) backstops that: installing past it reclaims
//! the oldest forwarder, so the map is bounded regardless of the webview's behavior.
//!
//! [`Channel`]: tauri::ipc::Channel

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use tauri::async_runtime::JoinHandle;

/// The most forwarders held at once. Each pooled terminal installs one and clears it on unmount,
/// so a healthy session holds at most a pool's worth (the frontend keeps ~6). This ceiling sits
/// well above that: it exists only to reclaim forwarders orphaned when the webview loses its
/// detach tokens, so the map can never grow without bound.
const MAX_FORWARDERS: usize = 16;

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
        // Reclaim forwarders orphaned by a webview reload or crash (their detach tokens were lost
        // in JS). Live pooled forwarders always hold the newest tokens, so the lowest token is
        // necessarily a leak — never a terminal the pane is still showing.
        while slots.forwarders.len() > MAX_FORWARDERS {
            let Some(&oldest) = slots.forwarders.keys().min() else {
                break;
            };
            if let Some(forwarder) = slots.forwarders.remove(&oldest) {
                forwarder.abort();
            }
        }
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
