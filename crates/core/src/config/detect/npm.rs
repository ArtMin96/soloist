//! npm/Node detection from `package.json` scripts.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{spec, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// Detects Node commands from `package.json` `scripts`. Only the well-known scripts Solo
/// prioritizes are surfaced — `dev`/`start`/`serve` (auto-started) then `build`/`test`
/// (offered, not auto-started) — so a freshly detected stack is signal, not every script.
/// Each becomes `npm run <script>`.
pub(super) struct Npm;

/// Dev-server scripts, pre-selected for auto-start.
const AUTO_START: &[&str] = &["dev", "start", "serve"];
/// One-shot scripts, offered without auto-start.
const MANUAL: &[&str] = &["build", "test"];

impl Detector for Npm {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        let Some(text) = files.read("package.json") else {
            return Vec::new();
        };
        let scripts = script_names(&text);
        let mut out = Vec::new();
        for name in AUTO_START {
            if scripts.contains(*name) {
                out.push(command(name, true));
            }
        }
        for name in MANUAL {
            if scripts.contains(*name) {
                out.push(command(name, false));
            }
        }
        out
    }
}

/// The `scripts` keys of a `package.json`. `package.json` is JSON, a subset of YAML, so
/// the existing YAML parser reads it; unknown top-level fields are ignored. Malformed
/// JSON yields no scripts rather than an error — detection is best-effort.
fn script_names(text: &str) -> std::collections::BTreeSet<String> {
    #[derive(Deserialize, Default)]
    struct PackageJson {
        #[serde(default)]
        scripts: BTreeMap<String, String>,
    }
    serde_norway::from_str::<PackageJson>(text)
        .map(|pkg| pkg.scripts.into_keys().collect())
        .unwrap_or_default()
}

fn command(script: &str, auto_start: bool) -> (String, ProcessSpec) {
    (
        script.to_string(),
        spec(format!("npm run {script}"), auto_start),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::detect::MapFiles;

    fn detect(package_json: &str) -> Vec<(String, ProcessSpec)> {
        Npm.detect(&MapFiles::new(&[("package.json", package_json)]))
    }

    #[test]
    fn no_package_json_detects_nothing() {
        assert!(Npm.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn dev_scripts_auto_start_and_come_first() {
        let detected = detect(r#"{"scripts":{"build":"vite build","dev":"vite","test":"vitest"}}"#);
        let names: Vec<&str> = detected.iter().map(|(n, _)| n.as_str()).collect();
        // dev (auto-start) is ordered before the one-shot build/test.
        assert_eq!(names, ["dev", "build", "test"]);
        assert!(detected[0].1.auto_start, "dev auto-starts");
        assert_eq!(detected[0].1.command, "npm run dev");
        assert!(!detected[1].1.auto_start, "build does not auto-start");
        assert!(!detected[2].1.auto_start, "test does not auto-start");
    }

    #[test]
    fn only_well_known_scripts_are_surfaced() {
        // `lint`/`prepare` are real scripts but not part of the prioritized set.
        let detected = detect(r#"{"scripts":{"lint":"eslint .","prepare":"husky"}}"#);
        assert!(detected.is_empty(), "non-prioritized scripts are ignored");
    }

    #[test]
    fn a_realistic_package_json_with_other_fields_parses() {
        let detected = detect(
            r#"{
              "name": "storefront",
              "version": "1.0.0",
              "scripts": { "dev": "vite", "build": "vite build" },
              "dependencies": { "react": "^19.0.0" }
            }"#,
        );
        let names: Vec<&str> = detected.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["dev", "build"]);
    }

    #[test]
    fn malformed_json_detects_nothing() {
        assert!(detect("{ not json").is_empty());
    }
}
