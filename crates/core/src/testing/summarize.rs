//! In-memory summarizer fakes: a [`CannedSummaryRunner`] returning a fixed reply, a
//! [`FailingSummaryRunner`] for the degradation path, and a [`FakeOutputSnapshot`] returning a
//! canned terminal tail — so the summary reactor is exercised headless, with no subprocess, no
//! supervisor, and no PTY.

use std::sync::Mutex;

use crate::agents::{OutputSnapshot, SummaryError, SummaryInvocation, SummaryRunner};
use crate::ids::ProcessId;
use crate::sync::lock;

/// A [`SummaryRunner`] returning a fixed reply for every invocation and recording the invocations
/// it was asked to run — so a test asserts the reactor composed and published a summary without
/// spawning a process.
pub struct CannedSummaryRunner {
    reply: String,
    seen: Mutex<Vec<SummaryInvocation>>,
}

impl CannedSummaryRunner {
    /// A runner that answers every invocation with `reply`.
    pub fn new(reply: impl Into<String>) -> Self {
        Self {
            reply: reply.into(),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Every invocation this runner was asked to run, in order.
    pub fn invocations(&self) -> Vec<SummaryInvocation> {
        lock(&self.seen).clone()
    }
}

impl SummaryRunner for CannedSummaryRunner {
    fn run(&self, invocation: &SummaryInvocation) -> Result<String, SummaryError> {
        lock(&self.seen).push(invocation.clone());
        Ok(self.reply.clone())
    }
}

/// A [`SummaryRunner`] that always fails, for exercising graceful degradation — no summary, no
/// panic, the core unaffected.
#[derive(Clone, Copy, Default)]
pub struct FailingSummaryRunner;

impl SummaryRunner for FailingSummaryRunner {
    fn run(&self, _invocation: &SummaryInvocation) -> Result<String, SummaryError> {
        Err(SummaryError::Failed(
            "summarizer unavailable in test".into(),
        ))
    }
}

/// An [`OutputSnapshot`] returning fixed rendered lines for any process — a canned terminal tail,
/// so a reactor test needs no supervisor or PTY.
pub struct FakeOutputSnapshot {
    lines: Vec<String>,
}

impl FakeOutputSnapshot {
    /// A snapshot source returning `lines` for every process.
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }
}

impl OutputSnapshot for FakeOutputSnapshot {
    fn recent_lines(&self, _id: ProcessId, _max_lines: usize) -> Vec<String> {
        self.lines.clone()
    }
}
