//! Soloist's domain core: bounded contexts, hexagonal port traits, domain types,
//! and the event bus.
//!
//! This crate is pure and framework-free — it imports no `tauri`, `rmcp`, `axum`,
//! or `rusqlite`. OS, UI, transport, and storage concerns live in adapter crates
//! behind ports; the dependency-direction check enforces this.
//!
//! The walking skeleton wires three live ports ([`ports::ProcessSpawner`],
//! [`ports::Clock`], [`ports::Store`]) and the event bus ([`events::EventBus`])
//! through a single [`facade::Facade`], proving the architecture end to end before
//! any feature lands.

// The core must not panic in long-running tasks: unwrap/expect/panic are denied
// outside test builds.
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod agents;
pub mod config;
pub mod coordination;
pub mod events;
pub mod facade;
pub mod identity;
pub mod idle;
pub mod ids;
pub mod metrics;
pub mod notify;
pub mod ports;
pub mod portscan;
pub mod process;
pub mod projects;
pub mod supervisor;
pub mod terminal;
pub mod trust;

mod sync;

#[cfg(test)]
mod testing;

pub use events::{DomainEvent, EventBus};
pub use facade::Facade;
pub use ids::{ProcessId, ProjectId};
pub use ports::{
    Clock, ExitFuture, ExitStatus, ProcessControl, ProcessSpawner, SpawnError, SpawnSpec, Spawned,
    Store, StoreError, TokioClock,
};
pub use process::{IllegalTransition, ProcStatus, ProcessKind, ProcessView};
