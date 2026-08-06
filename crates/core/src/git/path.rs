//! Whether a path a surface asked about names something inside the repository.
//!
//! Paths arrive from outside the core — a click on a tree row, a command over a socket — and
//! every read that follows joins one onto a project root. Checking it once, here, is what keeps
//! a path that climbs out of the project from reaching a filesystem through any of them.

use std::path::{Component, Path};

/// Whether `path` names something inside the repository: relative, naming only ordinary
/// components, and climbing out of the root at no point.
pub(super) fn inside_repository(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
