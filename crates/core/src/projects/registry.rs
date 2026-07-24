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

    /// The loaded project whose canonical root contains `path` — the project a caller runs *in*,
    /// resolved from its kernel-read working directory. The deepest (most specific) root that is an
    /// ancestor of, or equal to, `path` wins, so a project nested inside another resolves to the
    /// inner one. Containment is by whole path components ([`Path::starts_with`]), so a directory
    /// under `/p/trackler2` never matches a sibling project rooted at `/p/trackler`. `None` when no
    /// open project contains the path.
    ///
    /// `path` must be canonical and absolute: it is compared verbatim against the canonicalized
    /// stored roots ([`Self::add`]), so a path carrying symlinks or `..` could mis-match. The sole
    /// caller supplies the kernel-canonical `/proc/<pid>/cwd`, which already satisfies this — hence
    /// no `canonicalize` here (it is filesystem I/O, fails on a path that no longer exists, and does
    /// not belong in this pure registry lookup).
    pub fn project_at_path(&self, path: &Path) -> Result<Option<ProjectId>, StoreError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|record| path.starts_with(&record.root))
            .max_by_key(|record| record.root.components().count())
            .map(|record| record.id))
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

    #[test]
    fn project_at_path_resolves_the_containing_project() {
        // Seed sibling projects with the canonical roots the store would hold, then resolve a
        // working directory to the project whose root contains it — the directory signal that
        // scopes an agent Soloist did not launch.
        let repo = Arc::new(FakeProjectRepo::new());
        let alpha = repo
            .upsert(Path::new("/home/dev/alpha"), None, None)
            .expect("seed alpha");
        let beta = repo
            .upsert(Path::new("/home/dev/beta"), None, None)
            .expect("seed beta");
        let _gamma = repo
            .upsert(Path::new("/home/dev/gamma"), None, None)
            .expect("seed gamma");
        let projects = Projects::new(repo);

        // A directory inside a project's root resolves to that project.
        assert_eq!(
            projects
                .project_at_path(Path::new("/home/dev/beta/src"))
                .expect("resolve"),
            Some(beta.id),
        );
        // The root itself resolves to it.
        assert_eq!(
            projects
                .project_at_path(Path::new("/home/dev/alpha"))
                .expect("resolve"),
            Some(alpha.id),
        );
        // A directory under no project's root resolves to nothing.
        assert_eq!(
            projects
                .project_at_path(Path::new("/home/dev/elsewhere"))
                .expect("resolve"),
            None,
        );
    }

    #[test]
    fn project_at_path_is_component_wise_not_a_string_prefix() {
        // A project rooted at /p/trackler must never match a directory under the sibling
        // /p/trackler2, even though the first path is a string prefix of the second.
        let repo = Arc::new(FakeProjectRepo::new());
        let _trackler = repo
            .upsert(Path::new("/p/trackler"), None, None)
            .expect("seed trackler");
        let trackler2 = repo
            .upsert(Path::new("/p/trackler2"), None, None)
            .expect("seed trackler2");
        let projects = Projects::new(repo);

        assert_eq!(
            projects
                .project_at_path(Path::new("/p/trackler2/crates"))
                .expect("resolve"),
            Some(trackler2.id),
            "a cwd under /p/trackler2 resolves to trackler2, never the string-prefix sibling /p/trackler",
        );
    }

    #[test]
    fn project_at_path_picks_the_deepest_root_for_nested_projects() {
        // A project nested inside another resolves to the inner (most specific) one.
        let repo = Arc::new(FakeProjectRepo::new());
        let outer = repo
            .upsert(Path::new("/work/outer"), None, None)
            .expect("seed outer");
        let inner = repo
            .upsert(Path::new("/work/outer/inner"), None, None)
            .expect("seed inner");
        let projects = Projects::new(repo);

        assert_eq!(
            projects
                .project_at_path(Path::new("/work/outer/inner/src"))
                .expect("resolve"),
            Some(inner.id),
            "the deepest containing root wins",
        );
        assert_eq!(
            projects
                .project_at_path(Path::new("/work/outer/other"))
                .expect("resolve"),
            Some(outer.id),
            "a directory only the outer root contains resolves to the outer project",
        );
    }
}
