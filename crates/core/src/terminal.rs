//! Terminal I/O (context C3): per-process PTY output buffers, live output fan-out,
//! and the input channel that routes keystrokes and resizes to the owning actor.
//!
//! Each running process has one [`TerminalChannel`]: the owning actor writes raw PTY
//! bytes into shared, bounded [`TerminalBuffers`] and a live broadcast, while viewers
//! (the dashboard, MCP) read a rendered or raw snapshot and subscribe to the live
//! stream. The actor is the single writer; viewers only read — the same CQRS split the
//! event bus uses. Input flows the other way over a bounded channel, so a fast typist
//! or a paste applies backpressure rather than growing an unbounded queue.

mod buffers;
mod parser;
mod ring;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::{broadcast, mpsc};

use crate::ids::ProcessId;
use crate::ports::PtySize;
use crate::sync::lock;

use buffers::{ScrollbackBudget, TerminalBuffers};

/// A muted separator written into a process's terminal stream when it restarts, so the
/// retained output of the previous run is visually divided from the new run's. Styled
/// dim (ANSI faint) to match the calm dashboard surface; the rendered projection that
/// logs and MCP read keeps only its plain text (`──────────  restarted  ──────────`).
const RESTART_BANNER: &str = "\r\n\x1b[2m──────────  restarted  ──────────\x1b[0m\r\n";

/// Input channel depth: typed bytes and resizes buffered before the sender awaits.
/// Bounded so a paste burst applies backpressure instead of growing without limit.
const INPUT_CAPACITY: usize = 256;
/// Live output channel depth: chunks buffered per subscriber before it observes
/// `Lagged` and re-syncs from the scrollback snapshot.
const LIVE_CAPACITY: usize = 256;
/// How many trailing rendered lines a [`TerminalActivity`] snapshot carries — enough for
/// a heuristic to spot a short prompt at the tail without copying the whole scrollback.
const ACTIVITY_TAIL_LINES: usize = 8;

/// One line of rendered terminal output — escape sequences applied, not included. The
/// unit of the rendered scrollback that logs, search, and `get_process_output` read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LogLine {
    pub text: String,
}

/// A plain-text snapshot of a process's output: the retained scrollback lines plus the
/// in-progress current line. The byte-accurate stream for a true terminal emulator is
/// the raw scrollback ([`Terminals::scrollback`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RenderedScreen {
    pub lines: Vec<String>,
}

/// A chunk of raw PTY output, shared cheaply with every live viewer.
pub type PtyChunk = Arc<[u8]>;

/// A snapshot of a process's terminal liveness signals, read each sample by the agent idle
/// classifier (C4). It reports raw terminal facts only — leaving every interpretation
/// (busy/idle/permission, and how it differs per provider) to the agents context, so C3
/// stays free of agent policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalActivity {
    /// Monotonic count of bytes output so far; a change since the previous sample means
    /// the process produced output (the visible-output heuristic).
    pub output_seq: u64,
    /// The most recent OSC terminal title the process set, if any (the title heuristics).
    pub title: Option<String>,
    /// The last few rendered lines, for heuristics that inspect output content — e.g.
    /// spotting a permission prompt. Bounded to [`ACTIVITY_TAIL_LINES`].
    pub tail: Vec<String>,
}

/// A semantic event extracted from the PTY byte stream, surfaced for the owning actor
/// to publish as a domain event.
pub(crate) enum TerminalSignal {
    /// An OSC title set (the window/icon title).
    Title(String),
    /// A bell (`BEL`).
    Bell,
}

/// A request routed from a viewer to a running process's owning actor.
pub(crate) enum PtyInput {
    /// Bytes to write to the PTY (typed text or raw control sequences).
    Write(Vec<u8>),
    /// New terminal dimensions.
    Resize(PtySize),
}

/// The viewer-facing half of a process's terminal channel, held in the registry.
struct TerminalChannel {
    input: mpsc::Sender<PtyInput>,
    live: broadcast::Sender<PtyChunk>,
    buffers: Arc<Mutex<TerminalBuffers>>,
}

/// The owning actor's half of a process's terminal channel. The `input` receiver and
/// the `recorder` are separate fields so the actor's select loop can borrow the input
/// stream mutably while still recording output through the recorder.
pub(crate) struct ActorTerminal {
    pub(crate) input: mpsc::Receiver<PtyInput>,
    pub(crate) recorder: Recorder,
}

/// Writes a process's raw PTY output into the shared buffers and live broadcast that
/// viewers read. Held by the owning actor — the single writer to a process's output.
pub(crate) struct Recorder {
    live: broadcast::Sender<PtyChunk>,
    buffers: Arc<Mutex<TerminalBuffers>>,
}

impl Recorder {
    /// Records a chunk of raw PTY output: appends it to the bounded buffers, publishes
    /// it to live viewers, and returns the semantic signals (title, bell) it carried.
    ///
    /// The buffer append and the live publish happen under one lock so they are atomic
    /// with respect to [`Terminals::attach`] — an attaching viewer therefore sees a
    /// chunk in *either* the scrollback snapshot *or* the live stream, never both and
    /// never neither.
    pub(crate) fn record(&self, chunk: Vec<u8>) -> Vec<TerminalSignal> {
        let mut buffers = lock(&self.buffers);
        let signals = buffers.ingest(&chunk);
        // Best-effort: a process with no attached viewer simply has no live receivers.
        let _ = self.live.send(PtyChunk::from(chunk));
        signals
    }

    /// Marks a restart boundary in the output stream. When the process already has
    /// output retained from a previous run, writes a [`RESTART_BANNER`] into the buffers
    /// and the live stream so the kept output is divided from the new run's; a no-op on
    /// the first run, when there is nothing to separate. The banner is appended and
    /// published under one lock, with the same atomicity against [`Terminals::attach`]
    /// that [`Recorder::record`] guarantees.
    pub(crate) fn mark_restart(&self) {
        let mut buffers = lock(&self.buffers);
        if !buffers.has_output() {
            return;
        }
        let banner = RESTART_BANNER.as_bytes();
        buffers.ingest(banner);
        let _ = self.live.send(PtyChunk::from(banner.to_vec()));
    }
}

/// The registry of live terminal channels, keyed by process. Cloneable; all clones
/// share one map. An entry persists after its process stops so a stopped process's
/// scrollback stays readable; only its input/live halves go dead. A single
/// [`ScrollbackBudget`] shared by every channel bounds the aggregate raw scrollback
/// across all processes.
#[derive(Clone, Default)]
pub(crate) struct Terminals {
    inner: Arc<Mutex<HashMap<ProcessId, TerminalChannel>>>,
    budget: Arc<ScrollbackBudget>,
}

impl Terminals {
    /// Opens the terminal channel for `id`'s owning actor, returning the actor-facing
    /// half. A first launch creates fresh buffers and a fresh live broadcast; a
    /// *relaunch* — the same process restarting, including a crash auto-restart that
    /// spawns a new actor — reuses the existing buffers and live sender, so the output
    /// history survives the restart and an attached viewer stays subscribed across it.
    /// Only the input channel, whose receiver the previous actor owned, is replaced.
    pub(crate) fn open(&self, id: ProcessId) -> ActorTerminal {
        let (input_tx, input_rx) = mpsc::channel(INPUT_CAPACITY);
        let mut channels = lock(&self.inner);
        let (live, buffers) = match channels.get(&id) {
            Some(existing) => (existing.live.clone(), existing.buffers.clone()),
            None => (
                broadcast::channel(LIVE_CAPACITY).0,
                Arc::new(Mutex::new(TerminalBuffers::shared(self.budget.clone()))),
            ),
        };
        channels.insert(
            id,
            TerminalChannel {
                input: input_tx,
                live: live.clone(),
                buffers: buffers.clone(),
            },
        );
        ActorTerminal {
            input: input_rx,
            recorder: Recorder { live, buffers },
        }
    }

    /// A sender to route input to `id`'s actor, if it has a live channel.
    pub(crate) fn input(&self, id: ProcessId) -> Option<mpsc::Sender<PtyInput>> {
        lock(&self.inner).get(&id).map(|c| c.input.clone())
    }

    /// Attaches a viewer to `id`: atomically captures the raw scrollback and a live
    /// subscription so the replay has no gap or duplicate against the live stream. The
    /// caller replays the scrollback, then streams the receiver. `None` if the process
    /// has never been started.
    pub(crate) fn attach(&self, id: ProcessId) -> Option<(Vec<u8>, broadcast::Receiver<PtyChunk>)> {
        let map = lock(&self.inner);
        let channel = map.get(&id)?;
        let buffers = lock(&channel.buffers);
        let scrollback = buffers.raw();
        let receiver = channel.live.subscribe();
        Some((scrollback, receiver))
    }

    /// `id`'s raw byte scrollback snapshot. `None` if the process has never started.
    pub(crate) fn scrollback(&self, id: ProcessId) -> Option<Vec<u8>> {
        lock(&self.inner).get(&id).map(|c| lock(&c.buffers).raw())
    }

    /// `id`'s rendered output snapshot.
    pub(crate) fn rendered(&self, id: ProcessId) -> Option<RenderedScreen> {
        lock(&self.inner)
            .get(&id)
            .map(|c| lock(&c.buffers).rendered())
    }

    /// `id`'s terminal liveness snapshot for agent idle classification (C4). Reads the
    /// output counter, latest title, and rendered tail under one lock. `None` if the
    /// process has never been started.
    pub(crate) fn activity(&self, id: ProcessId) -> Option<TerminalActivity> {
        lock(&self.inner).get(&id).map(|channel| {
            let buffers = lock(&channel.buffers);
            TerminalActivity {
                output_seq: buffers.output_seq(),
                title: buffers.last_title(),
                tail: buffers.tail(ACTIVITY_TAIL_LINES),
            }
        })
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
