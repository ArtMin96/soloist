//! Agents settings (global Agents tab): the auto-summarization opt-in. The agent *tool registry*
//! itself (detect / add / edit / enable) lives in the C4 agents context (Phase 7) and is reused by
//! the tab, not duplicated here — this document holds only the summarization preference.
//!
//! Auto-summarization is OFF by default (a locked decision): the core must never hard-depend on an
//! LLM, so a summary is generated only when the user opts in by naming a summarizer tool here. With
//! no tool set, summarization stays off.

use serde::{Deserialize, Serialize};

/// The Agents tab document — the summarization opt-in only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    /// The agent tool used to generate a one-line summary when an agent or terminal goes idle, or
    /// `None` to keep auto-summarization off (the default). Names a tool from the C4 registry.
    pub summarizer_tool: Option<String>,
    /// The model passed to the summarizer tool (e.g. `haiku`), or `None` for the tool's default.
    pub summarizer_model: Option<String>,
}
