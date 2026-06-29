//! `solo://` links to coordination content (context C6): a stable, copy-pasteable handle to a
//! scratchpad or todo so a human can hand one to an agent ("read this").
//!
//! The link is `solo://proj/<project>/scratchpad|todo/<id>`, keyed by the **durable** ids
//! ([`ScratchpadId`]/[`TodoId`]) so it survives a rename and a restart. This module is the one place
//! the scheme and its segments are defined — the [`to_link`](Link::to_link)/[`parse`](Link::parse)
//! pair round-trips, and nothing else hand-writes the string. Resolving a parsed link to its content
//! (and refusing a foreign-scope or unknown one) is the façade's job, not this pure helper's.

use serde::{Deserialize, Serialize};

use super::scratchpad::ScratchpadView;
use super::todo::TodoView;
use crate::ids::{ProjectId, ScratchpadId, TodoId};

/// The URL scheme that identifies a Soloist coordination link.
const SCHEME: &str = "solo";
/// The fixed first path segment, naming the project space the id lives in.
const PROJECT_SEGMENT: &str = "proj";
/// The path segment selecting a scratchpad.
const SCRATCHPAD_SEGMENT: &str = "scratchpad";
/// The path segment selecting a todo.
const TODO_SEGMENT: &str = "todo";

/// What a [`Link`] points to within its project — a scratchpad or a todo, by durable id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    Scratchpad(ScratchpadId),
    Todo(TodoId),
}

/// A parsed `solo://` link: the project it is scoped to and the content it points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub project: ProjectId,
    pub target: LinkTarget,
}

/// Why a string could not be read as a `solo://` link — it is not in the
/// `solo://proj/<project>/scratchpad|todo/<id>` shape.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("not a valid solo:// link")]
pub struct LinkError;

impl Link {
    /// A link to scratchpad `id` in `project`.
    pub fn scratchpad(project: ProjectId, id: ScratchpadId) -> Self {
        Self {
            project,
            target: LinkTarget::Scratchpad(id),
        }
    }

    /// A link to todo `id` in `project`.
    pub fn todo(project: ProjectId, id: TodoId) -> Self {
        Self {
            project,
            target: LinkTarget::Todo(id),
        }
    }

    /// The canonical `solo://proj/<project>/scratchpad|todo/<id>` string for this link.
    pub fn to_link(&self) -> String {
        let (segment, id) = match self.target {
            LinkTarget::Scratchpad(id) => (SCRATCHPAD_SEGMENT, id.get()),
            LinkTarget::Todo(id) => (TODO_SEGMENT, id.get()),
        };
        format!(
            "{SCHEME}://{PROJECT_SEGMENT}/{}/{segment}/{id}",
            self.project.get()
        )
    }

    /// Parses a `solo://proj/<project>/scratchpad|todo/<id>` link, or [`LinkError`] if `input` is not
    /// in that shape (unknown scheme/segments, a non-numeric id, or extra path).
    pub fn parse(input: &str) -> Result<Self, LinkError> {
        let rest = input
            .strip_prefix(SCHEME)
            .and_then(|rest| rest.strip_prefix("://"))
            .ok_or(LinkError)?;
        let mut segments = rest.split('/');
        let mut next = || segments.next().ok_or(LinkError);

        if next()? != PROJECT_SEGMENT {
            return Err(LinkError);
        }
        let project = ProjectId::from_raw(parse_id(next()?)?);
        let kind = next()?;
        let id = parse_id(next()?)?;
        if segments.next().is_some() {
            return Err(LinkError);
        }
        let target = match kind {
            SCRATCHPAD_SEGMENT => LinkTarget::Scratchpad(ScratchpadId::from_raw(id)),
            TODO_SEGMENT => LinkTarget::Todo(TodoId::from_raw(id)),
            _ => return Err(LinkError),
        };
        Ok(Self { project, target })
    }
}

/// Whether `input` carries the `solo://` scheme — the cheap check an adapter uses to route a value
/// to the link resolver instead of treating it as a bare name/id. A scheme match that fails to fully
/// parse is still routed (and refused in the core) rather than mistaken for content.
pub fn is_link(input: &str) -> bool {
    input
        .strip_prefix(SCHEME)
        .is_some_and(|rest| rest.starts_with("://"))
}

/// Parses one path segment as a durable id (`u64`), rejecting an empty or non-numeric segment.
fn parse_id(segment: &str) -> Result<u64, LinkError> {
    segment.parse::<u64>().map_err(|_| LinkError)
}

/// What a resolved link yielded: the in-scope scratchpad or todo it points to. The façade composes
/// this after enforcing project scope, so a receiving agent reads the target through the same view
/// it would get from a direct read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkContent {
    Scratchpad(ScratchpadView),
    Todo(TodoView),
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
