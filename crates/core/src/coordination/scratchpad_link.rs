//! What a write says about a todo's optional association with the scratchpad it was derived from.

use serde::{Deserialize, Serialize};

/// What a write says about a todo's optional scratchpad association. Three explicit states rather
/// than a nested `Option`, so "said nothing" and "said none" can never be confused at a call site:
/// a caller that omits the association leaves whatever is there alone, and clearing it takes an
/// explicit [`Cleared`](ScratchpadLink::Cleared). Generic over the handle so the one shape serves
/// both a caller that addresses a scratchpad by `name` and the core, which stores its durable
/// [`ScratchpadId`](crate::ids::ScratchpadId).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScratchpadLink<T> {
    /// Leave the association exactly as it is — what a caller that did not mention it means.
    #[default]
    Unchanged,
    /// Drop the association, leaving the todo unlinked.
    Cleared,
    /// Point the todo at this scratchpad.
    Linked(T),
}

impl<T> ScratchpadLink<T> {
    /// The link a caller that always states the association explicitly asks for: a handle links,
    /// its absence clears. [`Unchanged`](Self::Unchanged) is unreachable this way — which is the
    /// point for a surface (a form, a create) whose association is never merely omitted.
    pub fn stated(handle: Option<T>) -> Self {
        match handle {
            Some(handle) => Self::Linked(handle),
            None => Self::Cleared,
        }
    }

    /// Replaces the handle with `resolve`'s output, keeping the link's state. Used to turn a link
    /// stated by name into one carrying the durable id, without the resolution failure collapsing
    /// into "no link".
    pub fn try_map<U, E>(
        self,
        resolve: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<ScratchpadLink<U>, E> {
        Ok(match self {
            Self::Unchanged => ScratchpadLink::Unchanged,
            Self::Cleared => ScratchpadLink::Cleared,
            Self::Linked(handle) => ScratchpadLink::Linked(resolve(handle)?),
        })
    }
}
