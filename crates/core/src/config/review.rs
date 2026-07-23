//! Building the per-command detail a trust review presents. The type itself is shared
//! vocabulary ([`crate::configchange`]); this is the config context's constructor for it.

use super::model::ProcessSpec;
use crate::configchange::TrustReviewCommand;

impl TrustReviewCommand {
    /// Builds the review detail for a named command from its spec.
    pub fn from_spec(name: &str, spec: &ProcessSpec) -> Self {
        Self {
            name: name.to_string(),
            variant_hash: spec.variant_hash().to_hex(),
            command: spec.command.clone(),
            working_dir: spec
                .working_dir
                .as_ref()
                .map(|dir| dir.to_string_lossy().into_owned()),
            env: spec.env.clone(),
        }
    }
}
