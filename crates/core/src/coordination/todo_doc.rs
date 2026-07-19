//! The small document every todo carries — the revision-guarded specification of the work, its
//! lifecycle status, and the size cap that bounds a write.

use serde::{Deserialize, Serialize};

/// The most text a todo's document may carry, summed across its fields, in bytes. A todo is a
/// work-item specification, not a document store; this bounds the persisted row so a runaway
/// caller cannot grow the table without limit. Tags, blockers, and comments are separate columns,
/// each mutated by its own operation, so they are not counted here.
pub const MAX_TODO_DOC_BYTES: usize = 64 * 1024;

/// A todo's lifecycle status — the label the owning agent declares, a closed set so it is never a
/// free-form string. Distinct from the *blocker gate*: an agent may mark a todo `Blocked` to
/// communicate, but what mechanically prevents completion is its unmet
/// [`blockers`](super::TodoView::blockers), not this label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    /// Not yet started.
    Open,
    /// Deliberately parked by the owner (a declared label, independent of the blocker gate).
    Blocked,
    /// Being worked on.
    InProgress,
    /// Finished. Reached only when every blocker is met (the gate).
    Done,
}

/// The small document every todo carries — the revision-guarded specification of the work: a title,
/// a free-form Markdown body, and the lifecycle status. The aggregate validates it on write
/// ([`validate`](TodoDoc::validate)). Tags, blockers, comments, and the lock are **not** part of the
/// document — they are live state mutated by their own operations, so a tag or comment change never
/// collides with a concurrent specification edit (mirroring the scratchpad split of body vs
/// tags/archived).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoDoc {
    /// A short imperative title — what this todo is.
    pub title: String,
    /// The free-form Markdown body: what needs doing and any detail a worker needs to act on it.
    pub body: String,
    /// The lifecycle status the owner declares.
    pub status: TodoStatus,
}

impl TodoDoc {
    /// Checks the write is well-formed: the title is not blank and the body stays within the size
    /// cap. The body may be blank — a blank document is valid; only the title and the size ceiling
    /// are enforced. Returns a single message naming every problem at once, or `Ok(())` when it is
    /// well-formed. The status is a closed enum, so it needs no validation.
    pub fn validate(&self) -> Result<(), String> {
        let mut problems: Vec<String> = Vec::new();
        if self.title.trim().is_empty() {
            problems.push("title must not be blank".to_owned());
        }
        if self.content_bytes() > MAX_TODO_DOC_BYTES {
            problems.push(format!(
                "the document exceeds the {} KiB cap",
                MAX_TODO_DOC_BYTES / 1024
            ));
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }

    /// The total bytes of the document's text — what a size cap bounds.
    fn content_bytes(&self) -> usize {
        self.title.len() + self.body.len()
    }
}
