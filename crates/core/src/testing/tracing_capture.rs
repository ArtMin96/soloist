//! Capturing what the core traced, for the paths whose whole point is not being silent.
//!
//! Some refusals are deliberately invisible to the domain — an illegal status transition, a wake
//! its owner would not take — and are traced instead, so a regression is diagnosable rather than a
//! silence. A test of one of those has nothing to assert unless it can see the trace, so it
//! installs this as the subscriber for the duration of the call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Records whether any `WARN` was emitted while it was installed — just enough to prove a path
/// traces rather than staying silent, without pulling in a subscriber implementation.
///
/// Install it around the code under test with [`tracing::subscriber::with_default`]. That is
/// thread-local, so the traced work has to happen on the calling thread: a path that only runs
/// inside a spawned task is tested by calling it directly rather than through the task.
#[derive(Clone, Default)]
pub struct WarnFlag(Arc<AtomicBool>);

impl WarnFlag {
    /// Whether a `WARN` was emitted while this was the active subscriber.
    pub fn was_warned(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl tracing::Subscriber for WarnFlag {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        if *event.metadata().level() == tracing::Level::WARN {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
