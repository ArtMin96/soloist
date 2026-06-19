//! Go module detection: a module root gets `go run .`.

use super::{spec, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// Detects a Go module: a `go.mod` gets `go run .`. Offered without auto-start — the
/// module may be a library with no `main` package, so the user opts in.
pub(super) struct Go;

impl Detector for Go {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        if files.read("go.mod").is_some() {
            vec![("run".to_string(), spec("go run .".to_string(), false))]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::detect::MapFiles;

    #[test]
    fn no_go_mod_detects_nothing() {
        assert!(Go.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn a_go_module_offers_go_run_without_auto_start() {
        let detected = Go.detect(&MapFiles::new(&[("go.mod", "module example.com/app\n")]));
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0, "run");
        assert_eq!(detected[0].1.command, "go run .");
        assert!(!detected[0].1.auto_start);
    }
}
