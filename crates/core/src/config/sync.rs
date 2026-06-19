//! The per-project `solo.yml` sync engine.
//!
//! On a re-read, the engine compares the file's content hash to the last-seen one
//! (cheap skip when a file is touched but unchanged), diffs the parsed config, asks
//! the [`TrustRepo`] whether any added or updated command's variant still needs
//! trust, and announces a [`DomainEvent::ConfigChanged`]. It owns **no** process
//! spawner, so a sync can update config and flag re-trust but can never start,
//! stop, or restart anything.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::diff::{diff, ConfigSync};
use super::load::{config_path, load_or_empty, ConfigError};
use super::model::{ProcessSpec, SoloYml};
use super::review::TrustReviewCommand;
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

    /// The current spec for a command by name in a loaded project, if present. Reads
    /// the last-synced snapshot — used to resolve a trust decision to a concrete
    /// variant (see [`crate::facade::Facade::trust_command`]).
    pub fn spec(&self, project: ProjectId, name: &str) -> Option<ProcessSpec> {
        let states = lock(&self.states);
        states.get(&project)?.last.processes.get(name).cloned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeTrustRepo;
    use tokio::sync::broadcast::error::TryRecvError;

    fn write(path: &std::path::Path, contents: &str) {
        std::fs::write(path, contents).expect("write solo.yml");
    }

    /// Builds an engine and seeds a project from an initial `solo.yml`, returning
    /// the engine, the trust repo, a fresh event receiver, the project id, and root.
    fn setup(
        initial: &str,
    ) -> (
        ConfigEngine,
        Arc<FakeTrustRepo>,
        tokio::sync::broadcast::Receiver<DomainEvent>,
        ProjectId,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().expect("temp dir");
        write(&config_path(dir.path()), initial);
        let trust = Arc::new(FakeTrustRepo::new());
        let bus = EventBus::new(16);
        let rx = bus.subscribe();
        let engine = ConfigEngine::new(trust.clone(), bus);
        let project = ProjectId::from_raw(1);
        engine
            .open(project, dir.path().to_path_buf())
            .expect("open seeds state");
        (engine, trust, rx, project, dir)
    }

    #[test]
    fn adding_a_command_emits_change_requiring_trust() {
        let (engine, _trust, mut rx, project, dir) =
            setup("processes:\n  Web:\n    command: npm run dev\n");
        write(
            &config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
        );

        let changes = engine.sync(project).expect("sync ok").expect("a change");
        assert_eq!(changes.added, vec!["Api"]);

        match rx.try_recv() {
            Ok(DomainEvent::ConfigChanged {
                requires_trust,
                diff,
                ..
            }) => {
                assert!(requires_trust, "a new untrusted command requires trust");
                assert_eq!(diff.added, vec!["Api"]);
            }
            other => panic!("expected ConfigChanged, got {other:?}"),
        }
    }

    #[test]
    fn the_change_event_carries_the_untrusted_command_detail() {
        let (engine, _trust, mut rx, project, dir) =
            setup("processes:\n  Web:\n    command: npm run dev\n");
        write(
            &config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n    working_dir: api\n    env:\n      PORT: '4000'\n",
        );

        engine.sync(project).expect("sync ok").expect("a change");

        match rx.try_recv() {
            Ok(DomainEvent::ConfigChanged { commands, .. }) => {
                assert_eq!(
                    commands.len(),
                    1,
                    "only the new untrusted command is pending"
                );
                let api = &commands[0];
                assert_eq!(api.name, "Api");
                assert_eq!(api.command, "cargo run");
                assert_eq!(api.working_dir.as_deref(), Some("api"));
                assert_eq!(api.env.get("PORT").map(String::as_str), Some("4000"));
            }
            other => panic!("expected ConfigChanged, got {other:?}"),
        }
    }

    #[test]
    fn renaming_a_trusted_command_preserves_trust() {
        let (engine, trust, mut rx, project, dir) =
            setup("processes:\n  Web:\n    command: npm run dev\n");
        // Trust Web's current variant.
        let web = crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").unwrap();
        trust
            .set_trusted(project, &web.processes["Web"].variant_hash())
            .unwrap();

        write(
            &config_path(dir.path()),
            "processes:\n  Frontend:\n    command: npm run dev\n",
        );
        let changes = engine.sync(project).expect("sync ok").expect("a change");
        assert_eq!(changes.renamed.len(), 1);
        assert!(changes.added.is_empty() && changes.removed.is_empty());

        match rx.try_recv() {
            Ok(DomainEvent::ConfigChanged { requires_trust, .. }) => {
                assert!(!requires_trust, "a pure rename keeps trust");
            }
            other => panic!("expected ConfigChanged, got {other:?}"),
        }
    }

    #[test]
    fn a_rename_that_also_edits_the_variant_still_requires_trust() {
        let (engine, trust, mut rx, project, dir) =
            setup("processes:\n  Web:\n    command: npm run dev\n");
        // Trust Web's original variant (no env).
        let original =
            crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").unwrap();
        trust
            .set_trusted(project, &original.processes["Web"].variant_hash())
            .unwrap();

        // Rename Web -> Frontend (same command, so it is detected as a rename) but
        // also add an env var — the variant changes, so trust must be re-confirmed.
        write(
            &config_path(dir.path()),
            "processes:\n  Frontend:\n    command: npm run dev\n    env:\n      PORT: '3000'\n",
        );
        let changes = engine.sync(project).expect("sync ok").expect("a change");
        assert_eq!(changes.renamed.len(), 1, "still classified as a rename");

        match rx.try_recv() {
            Ok(DomainEvent::ConfigChanged { requires_trust, .. }) => {
                assert!(
                    requires_trust,
                    "a rename that changes the variant needs re-trust"
                );
            }
            other => panic!("expected ConfigChanged, got {other:?}"),
        }
    }

    #[test]
    fn touching_without_changing_bytes_is_a_no_op() {
        let (engine, _trust, mut rx, project, dir) =
            setup("processes:\n  Web:\n    command: npm run dev\n");
        // Rewrite identical bytes.
        write(
            &config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n",
        );
        assert!(engine.sync(project).expect("sync ok").is_none());
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn syncing_an_unknown_project_is_a_no_op() {
        let (engine, _trust, _rx, _project, _dir) =
            setup("processes:\n  Web:\n    command: npm run dev\n");
        assert!(engine
            .sync(ProjectId::from_raw(999))
            .expect("sync ok")
            .is_none());
    }
}
