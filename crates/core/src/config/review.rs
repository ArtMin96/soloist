//! The per-command detail a trust review presents.

use std::collections::BTreeMap;

use serde::Serialize;

use super::model::ProcessSpec;

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

impl TrustReviewCommand {
    /// Builds the review detail for a named command from its spec.
    pub fn from_spec(name: &str, spec: &ProcessSpec) -> Self {
        Self {
            name: name.to_string(),
            command: spec.command.clone(),
            working_dir: spec
                .working_dir
                .as_ref()
                .map(|dir| dir.to_string_lossy().into_owned()),
            env: spec.env.clone(),
        }
    }
}
