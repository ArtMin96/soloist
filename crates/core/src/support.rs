//! Agent setup & support (context C8 edge): the usage guide the MCP surface serves, the
//! managed `AGENTS.md`/`CLAUDE.md` section writer, and locally stored feedback.
//!
//! Everything here exists so an agent can learn how to work inside Soloist and leave a
//! note for the user — none of it is process or coordination state. The guide content is
//! single-sourced as one topic set in [`guide`], so the `help` tool's overview and per-topic
//! answers, the server's onboarding instructions, and the section [`write_integration_guide`]
//! manages in a project file can never disagree.

mod feedback;
mod guide;
mod integration_file;

pub use feedback::{
    Feedback, FeedbackEntry, FeedbackError, FeedbackRepo, NoopFeedbackRepo, MAX_FEEDBACK_ENTRIES,
    MAX_FEEDBACK_LEN,
};
pub use guide::{agent_guide, help_overview, help_topic, onboarding_hint};
pub use integration_file::{
    write_integration_guide, IntegrationFile, IntegrationWrite, IntegrationWriteError,
};
