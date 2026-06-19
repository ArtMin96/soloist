//! Command auto-detection for a project opened without a `solo.yml`.
//!
//! Each ecosystem has one [`Detector`] (Strategy), registered once in [`DETECTORS`]
//! (Registry): adding an ecosystem is one new file plus one line here, never a growing
//! `match`. Detectors read project files through a [`FileSource`] and stay pure over
//! string inputs, so they unit-test without the filesystem; the disk read is a thin
//! shell ([`detect_in`]). Detection produces the core's own [`SoloYml`]/[`ProcessSpec`]
//! types — there is no parallel representation to keep in sync.

use std::path::Path;

use indexmap::IndexMap;

use super::model::{ProcessSpec, SoloYml};

mod npm;

/// Reads a project's files by path relative to its root. The real source reads from
/// disk ([`detect_in`]); tests pass an in-memory source, so detector logic never touches
/// the filesystem. A missing or non-UTF-8 file reads as `None`.
pub trait FileSource {
    /// The contents of `rel` (relative to the project root), or `None` when it does not
    /// exist or cannot be read as UTF-8.
    fn read(&self, rel: &str) -> Option<String>;
}

/// One ecosystem's detection strategy: inspect the project's files and contribute its
/// commands in display order. Pure over the [`FileSource`]; holds no state. An empty
/// result means "this ecosystem is not present."
pub trait Detector {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)>;
}

/// Every registered detector, in priority order. When two detectors contribute the same
/// command *name*, the earlier one wins (the later duplicate is dropped). Add an
/// ecosystem by adding its module above and one line here.
const DETECTORS: &[&dyn Detector] = &[&npm::Npm];

/// Runs every detector against `files` and merges their commands into a [`SoloYml`],
/// preserving detector order and dropping duplicate names. Pure — no filesystem access.
pub fn detect(files: &dyn FileSource) -> SoloYml {
    let mut processes: IndexMap<String, ProcessSpec> = IndexMap::new();
    for detector in DETECTORS {
        for (name, spec) in detector.detect(files) {
            processes.entry(name).or_insert(spec);
        }
    }
    SoloYml {
        name: None,
        icon: None,
        processes,
    }
}

/// Detects commands for the project at `root` by reading its files from disk — the thin
/// filesystem shell over the pure [`detect`].
pub fn detect_in(root: &Path) -> SoloYml {
    detect(&DiskFiles { root })
}

/// A [`FileSource`] backed by the real filesystem, rooted at a project directory.
struct DiskFiles<'a> {
    root: &'a Path,
}

impl FileSource for DiskFiles<'_> {
    fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(rel)).ok()
    }
}

#[cfg(test)]
use std::collections::HashMap;

/// An in-memory [`FileSource`] for detector unit tests.
#[cfg(test)]
pub(crate) struct MapFiles(HashMap<String, String>);

#[cfg(test)]
impl MapFiles {
    pub(crate) fn new(entries: &[(&str, &str)]) -> Self {
        Self(
            entries
                .iter()
                .map(|(path, body)| (path.to_string(), body.to_string()))
                .collect(),
        )
    }
}

#[cfg(test)]
impl FileSource for MapFiles {
    fn read(&self, rel: &str) -> Option<String> {
        self.0.get(rel).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_recognized_files_detects_nothing() {
        let files = MapFiles::new(&[("README.md", "# hi")]);
        assert!(detect(&files).processes.is_empty());
    }

    #[test]
    fn detect_in_reads_from_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"vite"}}"#,
        )
        .expect("write package.json");

        let config = detect_in(dir.path());
        assert_eq!(config.processes.len(), 1);
        assert_eq!(config.processes["dev"].command, "npm run dev");
    }
}
