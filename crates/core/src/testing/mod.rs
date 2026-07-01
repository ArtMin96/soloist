//! In-memory port fakes for headless testing, used by the core's own tests and — behind
//! the `testing` feature — by adapter-crate tests. A manually-advanced [`MockClock`], a
//! [`FakeSpawner`] whose children never touch the OS, a [`RecordingLockReleaser`],
//! in-memory [`FakeRuntimeState`]/[`FakeOrphanControl`] for orphan reconciliation,
//! [`FakeTrustRepo`]/[`FakeProjectRepo`]/[`FakeLockRepo`] standing in for the durable store, a
//! [`FakeAgentToolRepo`]/[`FakeVersionProbe`] for the agent registry and auto-detection, a
//! [`FakeMetricsProbe`]/[`FakePortProbe`] reporting fixed CPU-memory/port readings, a
//! [`FakeFileWatcher`] feeding synthetic filesystem changes, a [`RecordingNotifier`] capturing
//! the toasts the notification reactor composes, a [`CannedSummaryRunner`]/[`FailingSummaryRunner`]
//! plus a [`FakeOutputSnapshot`] for driving the summary reactor, the [`terminal_registration`]
//! fixture for driving the supervisor thread, and (in the core's own tests) the
//! `wait_all`/`next_matching` event waiters that let a test await an asynchronous effect
//! deterministically. Together they let
//! every actor transition, the grace window, panic isolation, the trust gate, and the
//! sync logic be exercised deterministically — no real time elapsed, no real processes
//! spawned, no SQLite. One submodule per cohesive concern; this root only re-exports them.

mod agents;
mod clock;
mod coordination;
mod coordination_kv;
mod coordination_todo;
// Event-stream waiters are used only by the core's own reactor tests, not by the adapter
// crates that consume the `testing` feature — and they assert via `panic!`, which the core
// denies outside test builds — so they compile under `cfg(test)` only.
#[cfg(test)]
mod events;
mod filewatch;
mod fixtures;
mod identity;
mod lock_releaser;
mod metrics;
mod notify;
mod portscan;
mod repos;
mod runtime_state;
mod settings;
mod shellenv;
mod spawner;
mod summarize;

pub use agents::{FakeAgentToolRepo, FakeVersionProbe};
pub use clock::MockClock;
pub use coordination::{FakeLockRepo, FakeScratchpadRepo, FakeTimerRepo};
pub use coordination_kv::FakeKvRepo;
pub use coordination_todo::FakeTodoRepo;
#[cfg(test)]
pub use events::{next_change, next_matching, next_to, wait_all};
pub use filewatch::FakeFileWatcher;
pub use fixtures::terminal_registration;
pub use identity::{authentic_session, TEST_PEER_PGID};
pub use lock_releaser::RecordingLockReleaser;
pub use metrics::FakeMetricsProbe;
pub use notify::RecordingNotifier;
pub use portscan::FakePortProbe;
pub use repos::{FakeProjectRepo, FakeTrustRepo};
pub use runtime_state::{FakeOrphanControl, FakeRuntimeState};
pub use settings::FakeSettingsRepo;
pub use shellenv::FakeShellEnvProbe;
pub use spawner::FakeSpawner;
pub use summarize::{CannedSummaryRunner, FailingSummaryRunner, FakeOutputSnapshot};
