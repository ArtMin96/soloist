//! Make detection: well-known targets become `make <target>`.

use super::{prioritized_targets, rule_names, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// The conventional Makefile names.
const MAKEFILES: &[&str] = &["Makefile", "makefile", "GNUmakefile"];

/// Detects Make targets: the well-known dev/build/test targets become `make <target>`
/// (dev targets auto-start). Target parsing is a best-effort line scan, not a full Make
/// parser.
pub(super) struct Make;

impl Detector for Make {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        let Some(text) = MAKEFILES.iter().find_map(|name| files.read(name)) else {
            return Vec::new();
        };
        prioritized_targets(&rule_names(&text), |target| format!("make {target}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::detect::MapFiles;

    #[test]
    fn no_makefile_detects_nothing() {
        assert!(Make.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn well_known_targets_become_make_commands() {
        let makefile = "\
CC := gcc
.PHONY: dev build
dev:
\tnpm run dev
build:
\tcargo build
lint:
\teslint .
";
        let detected = Make.detect(&MapFiles::new(&[("Makefile", makefile)]));
        let names: Vec<&str> = detected.iter().map(|(n, _)| n.as_str()).collect();
        // `dev` auto-starts and precedes `build`; the `CC :=` assignment and the
        // non-prioritized `lint` target are ignored.
        assert_eq!(names, ["dev", "build"]);
        assert_eq!(detected[0].1.command, "make dev");
        assert!(detected[0].1.auto_start);
        assert!(!detected[1].1.auto_start);
    }
}
