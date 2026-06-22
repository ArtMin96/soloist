//! In-memory port fakes for headless testing, used by the core's own tests and — behind
//! the `testing` feature — by adapter-crate tests. A manually-advanced [`MockClock`], a
//! [`FakeSpawner`] whose children never touch the OS, a [`RecordingLockReleaser`],
//! in-memory [`FakeRuntimeState`]/[`FakeOrphanControl`] for orphan reconciliation,
//! [`FakeTrustRepo`]/[`FakeProjectRepo`] standing in for the durable store, a
//! [`FakeAgentToolRepo`]/[`FakeVersionProbe`] for the agent registry and auto-detection, a
//! [`FakeMetricsProbe`]/[`FakePortProbe`] reporting fixed CPU-memory/port readings, a
//! [`FakeFileWatcher`] feeding synthetic filesystem changes, a [`RecordingNotifier`] capturing
//! the toasts the notification reactor composes, and the [`terminal_registration`]
//! fixture for driving the supervisor thread. Together they let
//! every actor transition, the grace window, panic isolation, the trust gate, and the
//! sync logic be exercised deterministically — no real time elapsed, no real processes
//! spawned, no SQLite. One submodule per cohesive concern; this root only re-exports them.

mod agents;
mod clock;
mod filewatch;
mod fixtures;
mod lock_releaser;
mod metrics;
mod notify;
mod portscan;
mod repos;
mod runtime_state;
mod spawner;

pub use agents::{FakeAgentToolRepo, FakeVersionProbe};
pub use clock::MockClock;
pub use filewatch::FakeFileWatcher;
pub use fixtures::terminal_registration;
pub use lock_releaser::RecordingLockReleaser;
pub use metrics::FakeMetricsProbe;
pub use notify::RecordingNotifier;
pub use portscan::FakePortProbe;
pub use repos::{FakeProjectRepo, FakeTrustRepo};
pub use runtime_state::{FakeOrphanControl, FakeRuntimeState};
pub use spawner::FakeSpawner;
