//! Writing a `solo.yml` — serialize the [`SoloYml`] model and prepend a plain-language
//! header. Used to auto-create a project's config from detected commands.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::detect::detect_in;
use super::load::config_path;
use super::model::SoloYml;

/// The header prepended to a generated `solo.yml`. `serde_norway` writes data, not
/// comments, so this explains the file to the person who opens it — in plain language,
/// since they may not have written it.
const HEADER: &str = "\
# solo.yml — Soloist's project file.
#
# Each entry under `processes:` is a command Soloist runs and watches for you:
# it starts and stops them, restarts them if they crash, and shows their output.
# Soloist created this file from what it found in this folder. Edit it freely —
# add commands, remove the ones you don't want, change a command, or set
# `auto_start: true` to launch one automatically when you open the project.

";

/// Why writing a `solo.yml` failed: serializing the model, or the file write itself.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("cannot serialize solo.yml: {0}")]
    Serialize(#[source] serde_norway::Error),
    #[error("cannot write {path}: {source}", path = path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Renders a [`SoloYml`] to file text: the header followed by the serialized model.
/// Round-trips: [`super::load::parse`] of the result is the input (the header is
/// comments, which parsing ignores; default fields are omitted by the model).
pub fn render(config: &SoloYml) -> Result<String, WriteError> {
    let body = serde_norway::to_string(config).map_err(WriteError::Serialize)?;
    Ok(format!("{HEADER}{body}"))
}

/// Auto-creates `solo.yml` in `root` from detected commands when none exists. Returns
/// `true` when it wrote a new file, `false` when one was already present — an existing
/// `solo.yml` (even an empty one) is never rewritten. The thin filesystem shell over
/// [`detect_in`] and [`render`]. The create is atomic (`O_EXCL`), so a file appearing
/// between the existence check and the write is never clobbered.
pub fn create_if_absent(root: &Path) -> Result<bool, WriteError> {
    let path = config_path(root);
    if path.exists() {
        return Ok(false);
    }
    let contents = render(&detect_in(root))?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(contents.as_bytes())
                .map_err(|source| WriteError::Write { path, source })?;
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(WriteError::Write { path, source }),
    }
}

#[cfg(test)]
mod tests {
    use super::super::load::parse;
    use super::super::model::ProcessSpec;
    use super::*;

    fn config_with(commands: &[(&str, &str, bool)]) -> SoloYml {
        let mut config = SoloYml::default();
        for (name, command, auto_start) in commands {
            config.processes.insert(
                name.to_string(),
                ProcessSpec {
                    command: command.to_string(),
                    working_dir: None,
                    auto_start: *auto_start,
                    auto_restart: false,
                    restart_when_changed: Vec::new(),
                    env: Default::default(),
                },
            );
        }
        config
    }

    #[test]
    fn render_carries_the_header_and_omits_default_fields() {
        let rendered = render(&config_with(&[
            ("dev", "npm run dev", true),
            ("build", "npm run build", false),
        ]))
        .expect("render");

        assert!(rendered.starts_with("# solo.yml"), "header is present");
        // Assert on the serialized data, not the header (whose plain-language guidance
        // mentions `auto_start: true` as an example).
        let body = rendered.strip_prefix(HEADER).expect("header prefix");
        // `auto_start` defaults true, so the dev command omits it; build (false) keeps it.
        assert!(body.contains("command: npm run dev"));
        assert!(!body.contains("auto_start: true"));
        assert!(body.contains("auto_start: false"));
        // Other defaults never appear.
        assert!(!body.contains("working_dir"));
        assert!(!body.contains("auto_restart"));
        assert!(!body.contains("env"));
    }

    #[test]
    fn render_round_trips_through_parse() {
        let original = config_with(&[("dev", "npm run dev", true), ("test", "npm test", false)]);
        let parsed = parse(&render(&original).expect("render")).expect("parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn an_empty_config_renders_and_parses_clean() {
        let rendered = render(&SoloYml::default()).expect("render");
        assert!(rendered.starts_with("# solo.yml"));
        assert!(parse(&rendered).expect("parse").processes.is_empty());
    }

    #[test]
    fn create_if_absent_writes_a_detected_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"vite"}}"#,
        )
        .expect("write package.json");

        assert!(create_if_absent(dir.path()).expect("create"));
        let written = parse(&std::fs::read_to_string(config_path(dir.path())).expect("read back"))
            .expect("parse");
        assert_eq!(written.processes["dev"].command, "npm run dev");
    }

    #[test]
    fn create_if_absent_writes_a_starter_when_nothing_is_detected() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(create_if_absent(dir.path()).expect("create"));
        let written =
            parse(&std::fs::read_to_string(config_path(dir.path())).expect("read")).expect("parse");
        assert!(written.processes.is_empty());
    }

    #[test]
    fn create_if_absent_never_rewrites_an_existing_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = config_path(dir.path());
        std::fs::write(&path, "processes:\n  web:\n    command: serve\n").expect("seed");
        let before = std::fs::read_to_string(&path).expect("read");

        assert!(!create_if_absent(dir.path()).expect("create"));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
    }
}
