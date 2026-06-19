//! The project registry: the set of workspace roots Soloist manages.
//!
//! A project is a filesystem folder. Its durable identity is its **canonical**
//! absolute path, so the same workspace is one project however its path was written
//! (symlinks, `.`/`..`, trailing slash). The durable [`ProjectId`] is assigned by
//! the store and is stable across runs — which is what lets trust persist.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::ids::ProjectId;
use crate::ports::{ProjectRecord, ProjectRepo, StoreError};

/// Registry over the durable [`ProjectRepo`].
pub struct Projects {
    repo: Arc<dyn ProjectRepo>,
}

impl Projects {
    /// Builds the registry over the durable project repository.
    pub fn new(repo: Arc<dyn ProjectRepo>) -> Self {
        Self { repo }
    }

    /// Adds (or refreshes the metadata of) the project rooted at `root`. The path is
    /// canonicalized to a stable absolute form first, so re-adding the same folder
    /// under a different spelling updates the one record rather than duplicating it.
    /// `name`/`icon` come from the project's `solo.yml`.
    pub fn add(
        &self,
        root: &Path,
        name: Option<&str>,
        icon: Option<&Path>,
    ) -> Result<ProjectRecord, ProjectError> {
        let canonical =
            std::fs::canonicalize(root).map_err(|source| ProjectError::Root { source })?;
        Ok(self.repo.upsert(&canonical, name, icon)?)
    }

    /// All known projects, most-recently-added first.
    pub fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        self.repo.list()
    }

    /// One project by id, `None` if absent.
    pub fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        self.repo.get(id)
    }

    /// The display projection of every known project, most-recently-added first — the
    /// project read model the UI groups its process tree by (snapshot half of
    /// snapshot-then-deltas; paired with [`crate::events::DomainEvent::ProjectOpened`]).
    pub fn views(&self) -> Result<Vec<ProjectView>, StoreError> {
        Ok(self.list()?.iter().map(ProjectView::from_record).collect())
    }

    /// Removes a project (and, by cascade in the store, its trust records).
    pub fn remove(&self, id: ProjectId) -> Result<(), StoreError> {
        self.repo.remove(id)
    }
}

/// A project's display identity for the UI read model — a projection of the durable
/// [`ProjectRecord`]. [`name`](Self::name) is always a human label: the `solo.yml`
/// `name:` when set, otherwise the project folder's name, so the sidebar can title a
/// project even when its config names none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
}

impl ProjectView {
    /// Projects a durable record into its display identity, resolving the name.
    pub fn from_record(record: &ProjectRecord) -> Self {
        Self {
            id: record.id,
            name: display_name(record),
            root: record.root.clone(),
        }
    }
}

/// A project's display name: its `solo.yml` `name:` if set and non-blank, else the
/// final component of its (canonical, absolute) root path — falling back to the whole
/// path only for a root with no final component.
fn display_name(record: &ProjectRecord) -> String {
    record
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            record
                .root
                .file_name()
                .unwrap_or(record.root.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
}

/// Why adding a project failed.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The root path could not be canonicalized (e.g. it does not exist).
    #[error("cannot resolve project root: {source}")]
    Root { source: std::io::Error },
    /// The durable store rejected the write.
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeProjectRepo;

    #[test]
    fn add_canonicalizes_and_dedupes_a_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let projects = Projects::new(Arc::new(FakeProjectRepo::new()));

        let first = projects.add(dir.path(), Some("app"), None).expect("add");
        // Re-adding the same folder via a `.`-laden spelling updates, not duplicates.
        let again = projects
            .add(&dir.path().join("."), Some("app-renamed"), None)
            .expect("re-add");
        assert_eq!(first.id, again.id);
        assert_eq!(projects.list().expect("list").len(), 1);
        assert_eq!(again.name.as_deref(), Some("app-renamed"));
    }

    #[test]
    fn missing_root_is_a_typed_error() {
        let projects = Projects::new(Arc::new(FakeProjectRepo::new()));
        let err = projects
            .add(Path::new("/no/such/path/soloist-test"), None, None)
            .unwrap_err();
        assert!(matches!(err, ProjectError::Root { .. }));
    }

    #[test]
    fn view_name_prefers_config_name_then_falls_back_to_the_folder() {
        // A blank or absent name falls back to the root's final component; a real name wins.
        let blank = ProjectRecord {
            id: ProjectId::from_raw(1),
            root: PathBuf::from("/projects/storefront"),
            name: Some("   ".to_string()),
            icon: None,
        };
        assert_eq!(ProjectView::from_record(&blank).name, "storefront");

        let named = ProjectRecord {
            name: Some("Storefront".to_string()),
            ..blank.clone()
        };
        assert_eq!(ProjectView::from_record(&named).name, "Storefront");

        let absent = ProjectRecord {
            name: None,
            ..blank
        };
        assert_eq!(ProjectView::from_record(&absent).name, "storefront");
    }

    #[test]
    fn views_project_the_known_records() {
        let dir = tempfile::tempdir().expect("temp dir");
        let projects = Projects::new(Arc::new(FakeProjectRepo::new()));
        let record = projects.add(dir.path(), Some("App"), None).expect("add");

        let views = projects.views().expect("views");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id, record.id);
        assert_eq!(views[0].name, "App");
        assert_eq!(views[0].root, record.root);
    }

    #[test]
    fn list_and_remove_round_trip() {
        let a = tempfile::tempdir().expect("temp dir a");
        let b = tempfile::tempdir().expect("temp dir b");
        let projects = Projects::new(Arc::new(FakeProjectRepo::new()));
        let pa = projects.add(a.path(), None, None).expect("add a");
        let _pb = projects.add(b.path(), None, None).expect("add b");
        assert_eq!(projects.list().expect("list").len(), 2);
        projects.remove(pa.id).expect("remove a");
        let remaining = projects.list().expect("list");
        assert_eq!(remaining.len(), 1);
        assert_ne!(remaining[0].id, pa.id);
    }
}
