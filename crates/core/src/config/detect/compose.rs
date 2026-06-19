//! Docker Compose detection: a compose file gets one `docker compose up`.

use super::{spec, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// The conventional Compose filenames, in the order Compose itself prefers.
const COMPOSE_FILES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// Detects a Docker Compose stack: any compose file yields a single `docker compose up`
/// that brings the whole stack up, auto-started.
pub(super) struct Compose;

impl Detector for Compose {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        if COMPOSE_FILES.iter().any(|name| files.read(name).is_some()) {
            vec![(
                "compose".to_string(),
                spec("docker compose up".to_string(), true),
            )]
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
    fn no_compose_file_detects_nothing() {
        assert!(Compose.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn a_compose_file_becomes_one_auto_started_command() {
        let detected = Compose.detect(&MapFiles::new(&[("docker-compose.yml", "services: {}")]));
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0, "compose");
        assert_eq!(detected[0].1.command, "docker compose up");
        assert!(detected[0].1.auto_start);
    }

    #[test]
    fn the_modern_compose_yaml_name_is_recognized() {
        let detected = Compose.detect(&MapFiles::new(&[("compose.yaml", "services: {}")]));
        assert_eq!(detected.len(), 1);
    }
}
