//! Soloist's domain core: bounded contexts, hexagonal port traits, domain types,
//! and the event bus.
//!
//! This crate is pure and framework-free — it imports no `tauri`, `rmcp`, `axum`,
//! or `rusqlite`. OS, UI, transport, and storage concerns live in adapter crates
//! behind ports; the dependency-direction check enforces this.

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
pub mod metrics;
pub mod notify;
pub mod ports;
pub mod projects;
pub mod supervisor;
pub mod terminal;
pub mod trust;
