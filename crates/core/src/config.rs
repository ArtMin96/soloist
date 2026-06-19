//! `solo.yml` (context C1): the data model, loader/validator, change diffing, and
//! the per-project sync engine.
//!
//! The model mirrors Solo's real schema byte-for-byte (top-level `name`/`icon`/
//! `processes`, the per-process fields). Loading is total — every failure is a typed
//! [`load::ConfigError`], never a panic. The [`sync::ConfigEngine`] turns a re-read
//! of `solo.yml` into a trust-aware [`diff::ConfigSync`] and announces it; it holds
//! no process spawner, so a sync can never start anything. [`detect`] auto-detects a
//! starting set of commands for a project opened without a `solo.yml`.

pub mod detect;
pub mod diff;
pub mod load;
pub mod model;
pub mod review;
pub mod sync;

pub use detect::detect_in;
pub use diff::{diff, ConfigSync, Rename};
pub use load::{
    config_path, load, load_or_empty, parse, ConfigError, CONFIG_FILENAME, MAX_CONFIG_BYTES,
};
pub use model::{ProcessSpec, SoloYml};
pub use review::TrustReviewCommand;
pub use sync::{ConfigEngine, SyncError};
