//! The vocabulary of a `solo.yml` change: what differed, and what now needs trust.
//!
//! Shared kernel, not a context. [`crate::events`] carries these on
//! [`ConfigChanged`](crate::events::DomainEvent::ConfigChanged), and the config context (C1) both
//! produces them and publishes that event — so if C1 owned them, the event bus and C1 would import
//! each other. They depend on nothing and live here instead, exactly as [`crate::process`] holds
//! [`ProcStatus`](crate::process::ProcStatus) for both `events` and the supervisor.
//!
//! Only the *types* moved. `crate::config` still computes them:
//! [`diff`](crate::config::diff) builds a [`ConfigSync`], and
//! [`TrustReviewCommand::from_spec`] builds a review row from a `ProcessSpec`.

use std::collections::BTreeMap;

use serde::Serialize;

/// A rename: the same command string moved from one process name to another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

/// The difference between a previous and current config, by process name. Carried
/// to adapters in [`crate::events::DomainEvent::ConfigChanged`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ConfigSync {
    /// Newly added process names.
    pub added: Vec<String>,
    /// Process names whose spec changed in place (any field).
    pub updated: Vec<String>,
    /// Removed process names.
    pub removed: Vec<String>,
    /// Unambiguous renames (a removed/added pair sharing one command string).
    pub renamed: Vec<Rename>,
}

impl ConfigSync {
    /// True when nothing changed between the two snapshots.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.renamed.is_empty()
    }
}

/// One command a [`crate::events::DomainEvent::ConfigChanged`] surfaces for trust
/// review: enough of the spec for the UI to show *what will run* — command, working
/// directory, and environment — before the user trusts it. `working_dir` is the raw
/// `solo.yml` value (relative to the project root, or `None` for the root).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrustReviewCommand {
    pub name: String,
    pub command: String,
    pub working_dir: Option<String>,
    pub env: BTreeMap<String, String>,
}
