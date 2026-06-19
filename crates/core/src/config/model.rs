//! The `solo.yml` data model and its documented defaults.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::hash::{Hash, Hasher};

/// A parsed `solo.yml`. Top-level keys mirror Solo's schema (`name`, `icon`,
/// `processes`); `processes` preserves the file's order via [`IndexMap`] so the
/// sidebar lists commands as written.
///
/// [`Serialize`] is the single source for writing a `solo.yml` (auto-detection,
/// [`super::write`]): `skip_serializing_if` omits fields left at their defaults so the
/// generated file stays minimal, and a round-trip through [`super::load::parse`] is the
/// identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoloYml {
    /// Optional display name shown on first load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional icon path, relative to the project root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PathBuf>,
    /// The managed commands, keyed by display name. Empty when the `processes` key
    /// is absent — an empty or comment-only file is a valid, empty config. Always
    /// written (even when empty) so a generated file shows the `processes:` key.
    #[serde(default)]
    pub processes: IndexMap<String, ProcessSpec>,
}

impl SoloYml {
    /// The icon path resolved against the project root, if any. Relative icon paths
    /// are interpreted relative to the root; absolute paths are returned unchanged.
    pub fn resolved_icon(&self, project_root: &Path) -> Option<PathBuf> {
        self.icon.as_ref().map(|icon| project_root.join(icon))
    }
}

/// One command definition from `solo.yml`. Field defaults encode our documented
/// decisions: `auto_start` defaults **true** (the `auto_start` schema gap — we
/// auto-start a project's stack); everything else defaults to off/empty.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessSpec {
    /// The shell command to run. Required.
    pub command: String,
    /// Working directory, relative to the project root. `None` means the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<PathBuf>,
    /// Whether to start this command when the project opens. Defaults to `true`, so it
    /// is written only when `false`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub auto_start: bool,
    /// Whether to relaunch after an unexpected exit. Defaults to `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_restart: bool,
    /// Globs (relative to the project root) whose changes trigger a restart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restart_when_changed: Vec<String>,
    /// Per-process environment overrides. A sorted map so the variant hash does not
    /// depend on declaration order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

fn default_true() -> bool {
    true
}

/// `skip_serializing_if` for a `bool` that defaults to `true`: omit it when still `true`.
fn is_true(value: &bool) -> bool {
    *value
}

/// `skip_serializing_if` for a `bool` that defaults to `false`: omit it when still `false`.
fn is_false(value: &bool) -> bool {
    !*value
}

impl ProcessSpec {
    /// The trust **variant key**: a collision-resistant digest over the fields that
    /// define a command's identity for trust — `command`, `working_dir`, and `env`.
    /// The process *name* is deliberately excluded, so renaming a command preserves
    /// its trust while editing the command, directory, or environment invalidates it.
    pub fn variant_hash(&self) -> Hash {
        let mut h = Hasher::new();
        h.field(self.command.as_bytes());
        match &self.working_dir {
            Some(dir) => {
                h.field(&[1]);
                h.field(dir.as_os_str().as_encoded_bytes());
            }
            None => {
                h.field(&[0]);
            }
        }
        h.field(&(self.env.len() as u64).to_le_bytes());
        for (key, value) in &self.env {
            h.field(key.as_bytes());
            h.field(value.as_bytes());
        }
        h.finish()
    }

    /// The working directory resolved against the project root. A relative
    /// `working_dir` is joined onto the root; `None` resolves to the root itself.
    pub fn resolved_working_dir(&self, project_root: &Path) -> PathBuf {
        match &self.working_dir {
            Some(dir) => project_root.join(dir),
            None => project_root.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(command: &str) -> ProcessSpec {
        ProcessSpec {
            command: command.to_string(),
            working_dir: None,
            auto_start: true,
            auto_restart: false,
            restart_when_changed: Vec::new(),
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn variant_hash_ignores_name_but_tracks_command_dir_env() {
        let base = spec("npm run dev");
        // Two specs with identical command/dir/env hash the same regardless of the
        // map key the caller stores them under — that is what preserves trust on
        // rename.
        assert_eq!(base.variant_hash(), spec("npm run dev").variant_hash());

        let edited = spec("npm run start");
        assert_ne!(base.variant_hash(), edited.variant_hash());

        let mut with_dir = base.clone();
        with_dir.working_dir = Some(PathBuf::from("web"));
        assert_ne!(base.variant_hash(), with_dir.variant_hash());

        let mut with_env = base.clone();
        with_env.env.insert("PORT".into(), "3000".into());
        assert_ne!(base.variant_hash(), with_env.variant_hash());
    }

    #[test]
    fn env_order_does_not_change_the_variant_hash() {
        let mut a = spec("run");
        a.env.insert("A".into(), "1".into());
        a.env.insert("B".into(), "2".into());
        let mut b = spec("run");
        b.env.insert("B".into(), "2".into());
        b.env.insert("A".into(), "1".into());
        assert_eq!(a.variant_hash(), b.variant_hash());
    }

    #[test]
    fn working_dir_resolves_against_the_root() {
        let root = Path::new("/projects/app");
        assert_eq!(
            spec("x").resolved_working_dir(root),
            PathBuf::from("/projects/app")
        );
        let mut nested = spec("x");
        nested.working_dir = Some(PathBuf::from("api"));
        assert_eq!(
            nested.resolved_working_dir(root),
            PathBuf::from("/projects/app/api")
        );
    }
}
