//! The supervisor's terminal output and input surface (context C2/C3): reading a process's
//! rendered/raw buffers and writing typed text or control sequences to its PTY. Split from
//! the root module to keep each file single-purpose; these all delegate to the per-process
//! [`Terminals`](crate::terminal::Terminals) the supervisor owns.

use tokio::sync::broadcast;

use crate::ids::ProcessId;
use crate::ports::PtySize;
use crate::terminal::{PtyChunk, PtyInput, RenderedScreen, TerminalActivity};

use super::{Supervisor, SupervisorError};

impl Supervisor {
    /// Attaches a viewer to a process's terminal output (detach/attach): returns the
    /// raw scrollback to replay plus a live receiver to stream, captured atomically so
    /// there is no gap or duplicate between them. `None` if the process has never been
    /// started. Detaching is just dropping the receiver — the process keeps running and
    /// other viewers are unaffected.
    pub fn attach_pty(&self, id: ProcessId) -> Option<(Vec<u8>, broadcast::Receiver<PtyChunk>)> {
        self.terminals.attach(id)
    }

    /// A process's raw byte scrollback snapshot (control sequences included), for output
    /// tools that read without attaching. `None` if it has never been started.
    pub fn pty_scrollback(&self, id: ProcessId) -> Option<Vec<u8>> {
        self.terminals.scrollback(id)
    }

    /// A process's rendered output snapshot (escape sequences applied to plain text).
    /// `None` if the process has never been started.
    pub fn rendered(&self, id: ProcessId) -> Option<RenderedScreen> {
        self.terminals.rendered(id)
    }

    /// A process's last `lines` rendered output lines — a bounded tail, not the whole
    /// scrollback. `None` if the process has never been started.
    pub fn rendered_tail(&self, id: ProcessId, lines: usize) -> Option<Vec<String>> {
        self.terminals.rendered_tail(id, lines)
    }

    /// A process's terminal liveness snapshot (output counter, latest title, rendered
    /// tail), read each sample by the agent idle classifier (C4). `None` if the process
    /// has never been started.
    pub fn terminal_activity(&self, id: ProcessId) -> Option<TerminalActivity> {
        self.terminals.activity(id)
    }

    /// Up to `limit` rendered output lines of `id` containing `query`. `None` if the
    /// process has never been started.
    pub fn search_output(&self, id: ProcessId, query: &str, limit: usize) -> Option<Vec<String>> {
        self.terminals.search_rendered(id, query, limit)
    }

    /// Up to `limit` raw output lines of `id` containing `query`. `None` if the process has
    /// never been started.
    pub fn search_raw_output(
        &self,
        id: ProcessId,
        query: &str,
        limit: usize,
    ) -> Option<Vec<String>> {
        self.terminals.search_raw(id, query, limit)
    }

    /// Clears `id`'s output buffers (rendered and raw) without stopping the process or
    /// touching its PTY. Returns whether the process had a terminal to clear.
    pub fn clear_output(&self, id: ProcessId) -> bool {
        self.terminals.clear(id)
    }

    /// Writes bytes (typed text or raw control sequences) to a running process's PTY.
    /// Returns [`SupervisorError::NotFound`] for a process with no terminal; input to a
    /// process that has since stopped is delivered best-effort and dropped.
    pub async fn write_stdin(&self, id: ProcessId, data: Vec<u8>) -> Result<(), SupervisorError> {
        self.send_input(id, PtyInput::Write(data)).await
    }

    /// Resizes a running process's PTY so the child sees the new dimensions (and a
    /// `SIGWINCH`). Best-effort, as for [`Supervisor::write_stdin`].
    pub async fn resize(&self, id: ProcessId, cols: u16, rows: u16) -> Result<(), SupervisorError> {
        self.send_input(id, PtyInput::Resize(PtySize { cols, rows }))
            .await
    }

    /// Routes one input message to a process's owning actor over its bounded input
    /// channel, applying backpressure rather than dropping when the actor is busy.
    async fn send_input(&self, id: ProcessId, input: PtyInput) -> Result<(), SupervisorError> {
        match self.terminals.input(id) {
            // A closed channel (the process has since stopped) is a harmless no-op.
            Some(sender) => {
                let _ = sender.send(input).await;
                Ok(())
            }
            None => Err(SupervisorError::NotFound(id)),
        }
    }
}
