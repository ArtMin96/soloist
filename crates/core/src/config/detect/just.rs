//! Just detection: well-known recipes become `just <recipe>`.

use super::{prioritized_targets, rule_names, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// The conventional justfile names.
const JUSTFILES: &[&str] = &["justfile", "Justfile", ".justfile"];

/// Detects Just recipes: the well-known dev/build/test recipes become `just <recipe>`
/// (dev recipes auto-start). Recipe parsing is a best-effort line scan, not a full Just
/// parser.
pub(super) struct Just;

impl Detector for Just {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        let Some(text) = JUSTFILES.iter().find_map(|name| files.read(name)) else {
            return Vec::new();
        };
        prioritized_targets(&rule_names(&text), |recipe| format!("just {recipe}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::detect::MapFiles;

    #[test]
    fn no_justfile_detects_nothing() {
        assert!(Just.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn well_known_recipes_become_just_commands() {
        // A recipe with a parameter still resolves to its leading name.
        let justfile = "\
serve port='8080':
    python -m http.server {{port}}
test:
    pytest
";
        let detected = Just.detect(&MapFiles::new(&[("justfile", justfile)]));
        let names: Vec<&str> = detected.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["serve", "test"]);
        assert_eq!(detected[0].1.command, "just serve");
        assert!(detected[0].1.auto_start);
        assert!(!detected[1].1.auto_start);
    }
}
