//! Cargo detection: a runnable crate gets `cargo run`.

use super::{spec, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// Detects a Rust crate: a `Cargo.toml` with a `[package]` section gets `cargo run`.
/// Offered without auto-start — a workspace-only manifest has no package, and detection
/// cannot tell a binary from a library without compiling, so the user opts in.
pub(super) struct Cargo;

impl Detector for Cargo {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        match files.read("Cargo.toml") {
            Some(text) if has_package_section(&text) => {
                vec![("run".to_string(), spec("cargo run".to_string(), false))]
            }
            _ => Vec::new(),
        }
    }
}

/// Whether the manifest declares a `[package]` (a buildable crate) rather than only a
/// `[workspace]`. A line-level check avoids a TOML-parser dependency.
fn has_package_section(manifest: &str) -> bool {
    manifest
        .lines()
        .map(str::trim)
        .any(|line| line == "[package]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::detect::MapFiles;

    fn detect(cargo_toml: &str) -> Vec<(String, ProcessSpec)> {
        Cargo.detect(&MapFiles::new(&[("Cargo.toml", cargo_toml)]))
    }

    #[test]
    fn no_manifest_detects_nothing() {
        assert!(Cargo.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn a_package_manifest_offers_cargo_run_without_auto_start() {
        let detected = detect("[package]\nname = \"app\"\nedition = \"2021\"\n");
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0, "run");
        assert_eq!(detected[0].1.command, "cargo run");
        assert!(
            !detected[0].1.auto_start,
            "an unknown binary does not auto-start"
        );
    }

    #[test]
    fn a_workspace_only_manifest_detects_nothing() {
        let detected = detect("[workspace]\nmembers = [\"crates/*\"]\n");
        assert!(detected.is_empty());
    }
}
