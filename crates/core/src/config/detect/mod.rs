//! Command auto-detection for a project opened without a `solo.yml`.
//!
//! Each ecosystem has one [`Detector`] (Strategy), registered once in [`DETECTORS`]
//! (Registry): adding an ecosystem is one new file plus one line here, never a growing
//! `match`. Detectors read project files through a [`FileSource`] and stay pure over
//! string inputs, so they unit-test without the filesystem; the disk read is a thin
//! shell ([`detect_in`]). Detection produces the core's own [`SoloYml`]/[`ProcessSpec`]
//! types — there is no parallel representation to keep in sync.

use std::collections::BTreeMap;
use std::path::Path;

use indexmap::IndexMap;

use super::model::{ProcessSpec, SoloYml};

mod cargo;
mod compose;
mod go;
mod just;
mod make;
mod npm;
mod procfile;

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
/// command *name*, the earlier one wins (the later duplicate is dropped) — so explicit
/// service/script declarations precede the generic task runners. Add an ecosystem by
/// adding its module above and one line here.
const DETECTORS: &[&dyn Detector] = &[
    &procfile::Procfile,
    &npm::Npm,
    &compose::Compose,
    &cargo::Cargo,
    &go::Go,
    &just::Just,
    &make::Make,
];

/// Runs every detector against `files` and merges their commands into a [`SoloYml`],
/// preserving detector order and dropping duplicate names. Pure — no filesystem access.
pub fn detect(files: &dyn FileSource) -> SoloYml {
    let mut processes: IndexMap<String, ProcessSpec> = IndexMap::new();
    for detector in DETECTORS {
        for (name, value) in detector.detect(files) {
            processes.entry(name).or_insert(value);
        }
    }
    SoloYml {
        name: None,
        icon: None,
        processes,
    }
}

/// A detected command's spec: the given shell command run from the project root, every
/// other field at its default. `auto_start` follows the detector's dev-vs-one-shot
/// policy (dev servers start on open; build/test do not).
pub(super) fn spec(command: String, auto_start: bool) -> ProcessSpec {
    ProcessSpec {
        command,
        working_dir: None,
        auto_start,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

/// Target/recipe names that run a long-lived process — pre-selected for auto-start.
const AUTO_TARGETS: &[&str] = &["dev", "start", "serve", "run"];
/// One-shot target/recipe names — offered without auto-start.
const MANUAL_TARGETS: &[&str] = &["build", "test"];

/// The rule names of a Make/Just-style file, in file order: the first token before a
/// `:` rule on a non-indented line, skipping comments and `:=`/`?=` assignments. Shared
/// by the Make and Just detectors.
pub(super) fn rule_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with(char::is_whitespace) {
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(colon) = line.find(':') else {
            continue;
        };
        // `name :=` is a variable assignment, not a rule (`name =`/`?=`/`+=` have no
        // colon and are skipped above). A `:` inside a recipe's parameter defaults is
        // fine — only an immediately-following `=` marks an assignment.
        if line.as_bytes().get(colon + 1) == Some(&b'=') {
            continue;
        }
        if let Some(name) = line[..colon].split_whitespace().next() {
            names.push(name.to_string());
        }
    }
    names
}

/// Maps a Make/Just rule-name list to prioritized commands via `command` (e.g.
/// `|t| format!("make {t}")`): the well-known dev targets first (auto-started), then
/// build/test (offered, not auto-started). Names outside the known set are ignored, so
/// a freshly detected stack is signal rather than every rule.
pub(super) fn prioritized_targets(
    names: &[String],
    command: impl Fn(&str) -> String,
) -> Vec<(String, ProcessSpec)> {
    let present = |target: &str| names.iter().any(|name| name == target);
    let mut out = Vec::new();
    for target in AUTO_TARGETS {
        if present(target) {
            out.push((target.to_string(), spec(command(target), true)));
        }
    }
    for target in MANUAL_TARGETS {
        if present(target) {
            out.push((target.to_string(), spec(command(target), false)));
        }
    }
    out
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

    #[test]
    fn duplicate_names_keep_the_higher_priority_detector() {
        // package.json (npm, higher priority) and a Makefile both define `build`; the
        // merged config keeps npm's, not make's.
        let files = MapFiles::new(&[
            ("package.json", r#"{"scripts":{"build":"vite build"}}"#),
            ("Makefile", "build:\n\tcargo build\n"),
        ]);
        let config = detect(&files);
        assert_eq!(config.processes["build"].command, "npm run build");
    }

    #[test]
    fn rule_names_skips_assignments_and_indented_recipe_lines() {
        let text = "\
CC := gcc
dev:
\techo dev
# a comment
build: dev
\techo build
";
        assert_eq!(rule_names(text), ["dev", "build"]);
    }

    #[test]
    fn prioritized_targets_order_dev_before_build_and_ignore_unknown() {
        let names = vec!["build".to_string(), "dev".to_string(), "lint".to_string()];
        let commands = prioritized_targets(&names, |t| format!("make {t}"));
        let labels: Vec<&str> = commands.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(labels, ["dev", "build"]);
        assert!(commands[0].1.auto_start);
        assert!(!commands[1].1.auto_start);
    }
}
