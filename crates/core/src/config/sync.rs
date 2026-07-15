//! The per-project `solo.yml` sync engine.
//!
//! On a re-read, the engine compares the file's content hash to the last-seen one
//! (cheap skip when a file is touched but unchanged), diffs the parsed config, asks
//! the [`TrustRepo`] whether any added or updated command's variant still needs
//! trust, and announces a [`DomainEvent::ConfigChanged`]. It owns **no** process
//! spawner, so a sync can update config and flag re-trust but can never start,
//! stop, or restart anything.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::diff::{diff, ConfigSync};
use super::edit;
use super::load::{config_path, load_or_empty, ConfigError, MAX_CONFIG_BYTES};
use super::model::{ProcessSpec, SoloYml};
use super::review::TrustReviewCommand;
use super::write::WriteError;
use crate::events::{DomainEvent, EventBus};
use crate::hash::{content_hash, Hash};
use crate::ids::ProjectId;
use crate::ports::{StoreError, TrustRepo};
use crate::sync::lock;

/// A failure while syncing a project's config: either reading/parsing the file or
/// consulting the trust store.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why an explicit `solo.yml` write failed. A duplicate or unknown command is reported by the
/// caller's mutation, and the file is never touched on any error.
#[derive(Debug, thiserror::Error)]
pub enum ConfigWriteError {
    /// The project is not open, so there is no `solo.yml` to write.
    #[error("no such project is open")]
    UnknownProject,
    /// The mutation named a command the config does not contain.
    #[error("no such command in the project config")]
    UnknownCommand,
    /// The mutation would add a command whose name is already taken — in the project's `solo.yml`
    /// or by an app-local command (a shared add refuses a name already used locally, so the two
    /// stores never collide).
    #[error("a command named {0:?} already exists")]
    DuplicateCommand(String),
    /// The icon path uses an unsupported image format (only png, jpg, gif, ico, webp are allowed).
    #[error("unsupported icon format {0:?} (use png, jpg, gif, ico, or webp)")]
    UnsupportedIcon(String),
    /// The command the mutation would store has no name or nothing to run.
    #[error(transparent)]
    InvalidCommand(#[from] crate::config::InvalidCommand),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Write(#[from] WriteError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The last-known state of one project's `solo.yml`.
struct ProjectState {
    root: PathBuf,
    last_hash: Hash,
    last: SoloYml,
}

/// Tracks each open project's `solo.yml` and turns a re-read into a trust-aware
/// change announcement. Cheap to share behind an `Arc`.
pub struct ConfigEngine {
    trust: Arc<dyn TrustRepo>,
    bus: EventBus,
    states: Mutex<HashMap<ProjectId, ProjectState>>,
}

impl ConfigEngine {
    /// Builds an engine over the durable trust store and the event bus.
    pub fn new(trust: Arc<dyn TrustRepo>, bus: EventBus) -> Self {
        Self {
            trust,
            bus,
            states: Mutex::new(HashMap::new()),
        }
    }

    /// Loads a project's `solo.yml` for the first time and seeds sync state. A
    /// missing file is an empty config. Returns the parsed config.
    pub fn open(&self, project: ProjectId, root: PathBuf) -> Result<SoloYml, ConfigError> {
        let (text, config) = load_or_empty(&config_path(&root))?;
        lock(&self.states).insert(
            project,
            ProjectState {
                root,
                last_hash: content_hash(text.as_bytes()),
                last: config.clone(),
            },
        );
        Ok(config)
    }

    /// Drops a project's sync state — the project-removal path evicting its last-seen
    /// `solo.yml`. A later [`Self::sync`] for the id reports no change and a
    /// [`Self::write`] refuses it as unknown; re-opening the project seeds fresh state
    /// via [`Self::open`]. The file on disk is untouched.
    pub fn forget(&self, project: ProjectId) {
        lock(&self.states).remove(&project);
    }

    /// Re-reads the project's `solo.yml`. When its content changed, diffs it against
    /// the last-seen config, determines whether any added/updated command needs
    /// re-trust, publishes a [`DomainEvent::ConfigChanged`] (unless the diff is
    /// empty), and updates sync state. Returns the diff when bytes changed, or
    /// `None` when the file is byte-identical or the project is unknown. Never
    /// starts a process.
    ///
    /// Drive this from a **single writer per project** (one debounced task). It reads
    /// sync state, releases the lock for blocking file I/O and the trust lookup, then
    /// writes new state back — so two concurrent calls for the same project can race
    /// the snapshot and double-publish [`DomainEvent::ConfigChanged`]. Because the
    /// I/O is blocking, an async caller must invoke it off-thread (e.g. via
    /// `spawn_blocking`) so it never stalls the runtime.
    pub fn sync(&self, project: ProjectId) -> Result<Option<ConfigSync>, SyncError> {
        let Some((root, prev_hash, prev)) = self.snapshot(project) else {
            return Ok(None);
        };

        let (text, config) = load_or_empty(&config_path(&root))?;
        let hash = content_hash(text.as_bytes());
        if hash == prev_hash {
            return Ok(None);
        }

        let changes = diff(&prev, &config);
        let commands = self.pending_trust(project, &config, &changes)?;
        let requires_trust = !commands.is_empty();

        lock(&self.states).insert(
            project,
            ProjectState {
                root,
                last_hash: hash,
                last: config,
            },
        );

        if !changes.is_empty() {
            self.bus.publish(DomainEvent::ConfigChanged {
                project,
                diff: changes.clone(),
                requires_trust,
                commands,
            });
        }
        Ok(Some(changes))
    }

    /// Applies `mutate` to a project's current `solo.yml` and writes the result back, preserving the
    /// file's comments and formatting where possible and **never** corrupting it (the write is
    /// re-parsed and verified, falling back to a faithful render otherwise). Then it refreshes sync
    /// state to the written bytes — so the file watcher's debounced re-read of our own write is a
    /// no-op (the hash matches) — and publishes a trust-aware [`DomainEvent::ConfigChanged`],
    /// returning the commands the change left needing trust (a shared add/edit re-trusts). A no-op
    /// mutation writes nothing; any error leaves the file untouched.
    ///
    /// Blocking file I/O — a settings write is small and infrequent, so the desktop adapter invokes
    /// it directly; a caller batching large writes should move it off-thread (`spawn_blocking`).
    /// Drive it from a single writer per project, like [`Self::sync`].
    pub fn write<F>(
        &self,
        project: ProjectId,
        mutate: F,
    ) -> Result<Vec<TrustReviewCommand>, ConfigWriteError>
    where
        F: FnOnce(&mut SoloYml) -> Result<(), ConfigWriteError>,
    {
        let Some((root, _, _)) = self.snapshot(project) else {
            return Err(ConfigWriteError::UnknownProject);
        };
        let path = config_path(&root);
        let (text, current) = load_or_empty(&path)?;

        let mut intended = current.clone();
        mutate(&mut intended)?;
        if intended == current {
            return Ok(Vec::new());
        }

        let new_text = edit::rewrite(&text, &current, &intended)?;
        // Never write past the read-side ceiling — a file that crossed it would be unreadable
        // (`ConfigError::TooLarge`) on the next load, and no write is unbounded.
        if new_text.len() as u64 > MAX_CONFIG_BYTES {
            return Err(ConfigWriteError::Config(ConfigError::TooLarge {
                path: path.clone(),
                size: new_text.len() as u64,
            }));
        }
        atomic_write(&path, &new_text)?;

        let hash = content_hash(new_text.as_bytes());
        let changes = diff(&current, &intended);
        let commands = self.pending_trust(project, &intended, &changes)?;
        let requires_trust = !commands.is_empty();
        lock(&self.states).insert(
            project,
            ProjectState {
                root,
                last_hash: hash,
                last: intended,
            },
        );
        if !changes.is_empty() {
            self.bus.publish(DomainEvent::ConfigChanged {
                project,
                diff: changes,
                requires_trust,
                commands: commands.clone(),
            });
        }
        Ok(commands)
    }

    /// The current spec for a command by name in a loaded project, if present. Reads
    /// the last-synced snapshot — used to resolve a trust decision to a concrete
    /// variant (see [`crate::facade::Facade::trust_command`]).
    pub fn spec(&self, project: ProjectId, name: &str) -> Option<ProcessSpec> {
        let states = lock(&self.states);
        states.get(&project)?.last.processes.get(name).cloned()
    }

    /// The last-synced `solo.yml` for a loaded project, `None` when the project is not open. The
    /// shared config as the engine last read or wrote it — the settings page reads this to list a
    /// project's shared commands without touching the filesystem.
    pub fn current(&self, project: ProjectId) -> Option<SoloYml> {
        lock(&self.states).get(&project).map(|s| s.last.clone())
    }

    fn snapshot(&self, project: ProjectId) -> Option<(PathBuf, Hash, SoloYml)> {
        let states = lock(&self.states);
        let state = states.get(&project)?;
        Some((state.root.clone(), state.last_hash, state.last.clone()))
    }

    /// The commands a change touched (added, updated, or rename target) whose current
    /// variant is not trusted — exactly what the review dialog offers to trust, with
    /// the detail to show what each will run. Checking by *variant* is what makes a
    /// pure rename free — its target variant equals the source's, already trusted — so
    /// it does not appear here, while a rename that also edits command/dir/env (a new
    /// variant) correctly does.
    fn pending_trust(
        &self,
        project: ProjectId,
        config: &SoloYml,
        changes: &ConfigSync,
    ) -> Result<Vec<TrustReviewCommand>, StoreError> {
        let touched = changes
            .added
            .iter()
            .chain(changes.updated.iter())
            .chain(changes.renamed.iter().map(|rename| &rename.to));
        let mut pending = Vec::new();
        for name in touched {
            if let Some(spec) = config.processes.get(name) {
                if !self.trust.is_trusted(project, &spec.variant_hash())? {
                    pending.push(TrustReviewCommand::from_spec(name, spec));
                }
            }
        }
        Ok(pending)
    }
}

/// Writes `contents` to `path` atomically: a sibling temp file is written and then renamed over the
/// target, so a crash mid-write can never leave a half-written `solo.yml` (rename is atomic on the
/// same filesystem).
fn atomic_write(path: &Path, contents: &str) -> Result<(), WriteError> {
    let tmp = path.with_extension("yml.tmp");
    std::fs::write(&tmp, contents).map_err(|source| WriteError::Write {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, path).map_err(|source| WriteError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
