//! The commit history a surface reads, one bounded page at a time.
//!
//! A history has no end worth reading to, so it is never asked for whole. A page is what a caller
//! gets, and a caller that wants more asks for the next one — which is the same shape whether it is
//! feeding a list somebody scrolls or a handful of recent subjects somebody else is about to learn
//! the house voice from.

use std::path::Path;

use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::CommitEntry;

use super::error::GitError;
use super::repository::LogRange;
use super::status::Git;

/// The most commits one page carries. Past this a reader is scrolling, not reading, and a surface
/// that wanted them all would be holding a whole repository's history in memory to show forty rows.
pub const LOG_PAGE_SIZE: usize = 50;

impl Git {
    /// One page of `project`'s history over `range`, newest first: `skip` commits passed over, at
    /// most `limit` returned, and `limit` is itself capped at [`LOG_PAGE_SIZE`] so no caller can ask
    /// for the whole of it.
    ///
    /// `None` for a root that is not a repository, as every other read here reports it. An **empty**
    /// list is different and ordinary: a repository with no commits yet has a history, and it is
    /// empty — as is a branch holding nothing its base does not.
    ///
    /// Reads are ungated — looking at what was committed runs nothing the repository carries. Runs
    /// an external tool, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn history(
        &self,
        project: ProjectId,
        root: &Path,
        range: LogRange<'_>,
        skip: usize,
        limit: usize,
    ) -> Result<Option<Vec<CommitEntry>>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        match self
            .repository
            .log(root, range, skip, limit.min(LOG_PAGE_SIZE))
        {
            Ok(page) => Ok(Some(page)),
            Err(GitError::NotARepo) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
