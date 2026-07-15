//! Agent idle detection (part of context C4): classifying each running agent into the
//! five-state [`AgentActivity`] from its terminal output.
//!
//! This is the observable substrate the coordination layer needs — a way to know, without a
//! human watching, whether an agent is busy, available, or blocked. It answers two questions:
//! *busy or available?* and *does it need a human?* (see [`AgentActivity`]). There is no
//! universal idle signal, so a [`strategy`] per provider reads whichever cue that CLI exposes
//! (visible output, OSC-title stability, or OSC-title status). A [`classifier`] turns each
//! agent's samples into edge-triggered transitions, the [`tracker`] holds the live set, and
//! the [`sampler`] drives it on a clock and publishes the transitions.
//!
//! The result only *informs* — it drives notifications and (later) fire-when-idle timers; it
//! never auto-acts, because the signal is a heuristic ("a quiet terminal is not always
//! completed work").

mod classifier;
mod permission;
mod sampler;
mod strategy;
mod tracker;

pub use crate::idle::AgentActivity;
pub use sampler::IdleSampler;
pub use tracker::IdleTracker;
