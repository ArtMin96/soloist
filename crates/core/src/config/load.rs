//! Reading, parsing, and validating a `solo.yml` into a [`SoloYml`].

use std::path::{Path, PathBuf};

use super::model::SoloYml;

/// Maximum `solo.yml` size — 1 MiB (Solo limits the file to "1 MB"). Larger files
/// are rejected rather than parsed, bounding the work a single config can cause.
pub const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// The conventional config filename within a project root.
pub const CONFIG_FILENAME: &str = "solo.yml";

/// The `solo.yml` path within a project root.
pub fn config_path(root: &Path) -> PathBuf {
    root.join(CONFIG_FILENAME)
}

/// Why loading or validating a `solo.yml` failed. Every variant is a value an
/// adapter can render; loading never panics.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The file could not be read.
    #[error("cannot read {path}: {source}", path = path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file is larger than [`MAX_CONFIG_BYTES`].
    #[error("{path} is {size} bytes, over the {max}-byte limit", path = path.display(), max = MAX_CONFIG_BYTES)]
    TooLarge { path: PathBuf, size: u64 },
    /// The YAML is malformed or violates the schema (unknown field, missing
    /// `command`, wrong type). The message carries the parser's line/column detail.
    #[error("invalid solo.yml: {0}")]
    Parse(String),
    /// A `restart_when_changed` entry was empty/whitespace-only.
    #[error("process {process:?}: restart_when_changed contains an empty glob")]
    EmptyGlob { process: String },
}

/// Parses `solo.yml` text into a [`SoloYml`]. Pure (no filesystem), so it is
/// trivially unit-testable; [`load`] layers the size limit and I/O on top.
///
/// An empty or comment-only document is a valid, empty config.
pub fn parse(text: &str) -> Result<SoloYml, ConfigError> {
    if !has_meaningful_content(text) {
        return Ok(SoloYml::default());
    }
    let config: SoloYml =
        serde_norway::from_str(text).map_err(|err| ConfigError::Parse(err.to_string()))?;
    validate(&config)?;
    Ok(config)
}

/// Reads and parses `solo.yml` at `path`, enforcing [`MAX_CONFIG_BYTES`]. A missing
/// file is an error; use [`load_or_empty`] where absence should mean "no commands".
pub fn load(path: &Path) -> Result<SoloYml, ConfigError> {
    Ok(read_and_parse(path)?.1)
}

/// Like [`load`], but a missing file yields an empty config — the meaning sync
/// gives an absent `solo.yml`. Returns the raw text alongside the parsed config so
/// callers can content-hash exactly what was on disk.
pub fn load_or_empty(path: &Path) -> Result<(String, SoloYml), ConfigError> {
    match read_and_parse(path) {
        Ok(pair) => Ok(pair),
        Err(ConfigError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok((String::new(), SoloYml::default()))
        }
        Err(err) => Err(err),
    }
}

fn read_and_parse(path: &Path) -> Result<(String, SoloYml), ConfigError> {
    let metadata = std::fs::metadata(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(ConfigError::TooLarge {
            path: path.to_path_buf(),
            size: metadata.len(),
        });
    }
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let config = parse(&text)?;
    Ok((text, config))
}

fn validate(config: &SoloYml) -> Result<(), ConfigError> {
    for (name, spec) in &config.processes {
        if spec
            .restart_when_changed
            .iter()
            .any(|glob| glob.trim().is_empty())
        {
            return Err(ConfigError::EmptyGlob {
                process: name.clone(),
            });
        }
    }
    Ok(())
}

fn has_meaningful_content(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .any(|line| !line.is_empty() && !line.starts_with('#'))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verbatim example from Solo's documented `solo.yml` schema.
    const REFERENCE_YML: &str = "\
name: storefront
icon: assets/project-icon.png
processes:
  Web:
    command: npm run dev
    working_dir: web
    auto_start: true
    auto_restart: false
    restart_when_changed: []
    env: {}
";

    #[test]
    fn parses_the_reference_example_with_correct_fields() {
        let config = parse(REFERENCE_YML).expect("reference config parses");
        assert_eq!(config.name.as_deref(), Some("storefront"));
        assert_eq!(config.processes.len(), 1);
        let web = &config.processes["Web"];
        assert_eq!(web.command, "npm run dev");
        assert_eq!(
            web.working_dir.as_deref(),
            Some(std::path::Path::new("web"))
        );
        assert!(web.auto_start);
        assert!(!web.auto_restart);
    }

    #[test]
    fn auto_start_defaults_true_when_omitted() {
        let config = parse("processes:\n  Web:\n    command: npm run dev\n").expect("parses");
        let web = &config.processes["Web"];
        assert!(web.auto_start, "auto_start should default to true");
        assert!(!web.auto_restart, "auto_restart should default to false");
        assert!(web.restart_when_changed.is_empty());
        assert!(web.env.is_empty());
    }

    #[test]
    fn empty_or_comment_only_files_are_empty_configs() {
        assert!(parse("").expect("empty parses").processes.is_empty());
        assert!(parse("   \n\t\n")
            .expect("whitespace parses")
            .processes
            .is_empty());
        assert!(parse("# just a comment\n# another\n")
            .expect("comments parse")
            .processes
            .is_empty());
    }

    #[test]
    fn unknown_field_is_a_named_error() {
        let err = parse("processes:\n  Web:\n    command: x\n    bogus: 1\n").unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("bogus"),
            "error should name the bad field: {message}"
        );
    }

    #[test]
    fn missing_command_is_an_error() {
        let err = parse("processes:\n  Web:\n    auto_start: true\n").unwrap_err();
        assert!(
            err.to_string().contains("command"),
            "error should mention command"
        );
    }

    #[test]
    fn empty_glob_is_rejected() {
        let err = parse("processes:\n  Web:\n    command: x\n    restart_when_changed: ['']\n")
            .unwrap_err();
        assert!(matches!(err, ConfigError::EmptyGlob { .. }));
    }

    #[test]
    fn load_enforces_the_size_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("solo.yml");
        let oversize = "#".repeat((MAX_CONFIG_BYTES + 1) as usize);
        std::fs::write(&path, oversize).expect("write oversize");
        match load(&path) {
            Err(ConfigError::TooLarge { size, .. }) => assert!(size > MAX_CONFIG_BYTES),
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn load_or_empty_treats_a_missing_file_as_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (text, config) =
            load_or_empty(&dir.path().join("absent.yml")).expect("missing is empty");
        assert!(text.is_empty());
        assert!(config.processes.is_empty());
    }
}
