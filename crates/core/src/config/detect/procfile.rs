//! Procfile detection: each `name: command` line is a service.

use super::{spec, Detector, FileSource};
use crate::config::model::ProcessSpec;

/// Detects services from a `Procfile`. Every `name: command` line is a long-lived
/// service, so each detected process auto-starts.
pub(super) struct Procfile;

impl Detector for Procfile {
    fn detect(&self, files: &dyn FileSource) -> Vec<(String, ProcessSpec)> {
        let Some(text) = files.read("Procfile") else {
            return Vec::new();
        };
        text.lines().filter_map(parse_line).collect()
    }
}

/// Parses one `name: command` line into a service, or `None` for a blank/comment line or
/// one missing a single-word name or a command.
fn parse_line(line: &str) -> Option<(String, ProcessSpec)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (name, command) = line.split_once(':')?;
    let (name, command) = (name.trim(), command.trim());
    if name.is_empty() || command.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    Some((name.to_string(), spec(command.to_string(), true)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::detect::MapFiles;

    fn detect(procfile: &str) -> Vec<(String, ProcessSpec)> {
        Procfile.detect(&MapFiles::new(&[("Procfile", procfile)]))
    }

    #[test]
    fn no_procfile_detects_nothing() {
        assert!(Procfile.detect(&MapFiles::new(&[])).is_empty());
    }

    #[test]
    fn each_service_line_becomes_an_auto_started_process() {
        let detected = detect("web: npm run start\nworker: node worker.js\n");
        let names: Vec<&str> = detected.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["web", "worker"]);
        assert_eq!(detected[0].1.command, "npm run start");
        assert!(detected[0].1.auto_start);
        assert!(detected[1].1.auto_start);
    }

    #[test]
    fn blank_and_comment_lines_are_skipped() {
        let detected = detect("\n# the web server\nweb: serve\n");
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].0, "web");
    }

    #[test]
    fn a_line_without_a_command_is_skipped() {
        assert!(detect("web:\n").is_empty());
    }
}
