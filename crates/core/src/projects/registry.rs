//! The durable project registry: the set of workspace roots Soloist manages.

use std::path::Path;
use std::sync::Arc;

use crate::ids::ProjectId;
use crate::ports::{ProjectRecord, ProjectRepo, StoreError};
use crate::projects::ProjectView;

/// Registry over the durable [`ProjectRepo`]. A project is a filesystem folder whose
/// durable identity is its canonical absolute path; the store assigns a stable
/// [`ProjectId`] from that path, which is what lets trust persist across runs.
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

    /// The durable repository behind the registry, for a caller in this context that must run a
    /// store call from async. The handle is cheap to clone and `'static`, so it can move into a
    /// blocking task instead of parking a runtime worker on the call.
    pub(super) fn repo(&self) -> Arc<dyn ProjectRepo> {
        Arc::clone(&self.repo)
    }
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
