//! Assist settings: which configured agent tool Soloist may run, headless and once, to draft text
//! when the user asks it to.
//!
//! Opt-in by construction. The document's default selects nothing, so on a fresh install — and on
//! every install upgrading into this — nothing is ever run, and the affordances that would use a
//! tool are not offered at all. Picking one is the whole act of enabling the feature; no credential
//! and no key is involved, because what runs is the user's own already-configured CLI.

use serde::{Deserialize, Serialize};

/// The Assist document. `tool` is the registry name of the [`AgentTool`](crate::agents::AgentTool)
/// that drafts text — the same unique name the launch picker resolves — or `None` for no assistance
/// at all, which is the default.
///
/// A name rather than a copy of the definition: the registry is the single source for what a tool
/// is, so editing a tool's command changes what a draft runs without anything here being rewritten.
/// A name that no longer resolves is refused where the draft is asked for, rather than silently
/// running something else.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Assist {
    pub tool: Option<String>,
}
