use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};

/// A façade over in-memory fakes with the given project repo.
fn facade_over(projects: Arc<FakeProjectRepo>) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .build(),
    )
}

/// `whoami` keeps the resolved effective-project id even when the durable store cannot be read for
/// its name. The scope is resolved from in-memory identity and stays enforced, so a transient store
/// fault must dim the name to `None` rather than reporting the scope as unresolved — otherwise an
/// agent whose scope is intact could wrongly conclude it lost its scope and refuse a scoped tool.
#[test]
fn whoami_keeps_the_scope_id_when_the_project_name_cannot_be_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let record = projects
        .upsert(dir.path(), Some("storefront"), None)
        .expect("seed one project");
    let facade = facade_over(projects.clone());
    // The sole loaded project is the unbound session's default scope.
    let session = facade.open_session(None);

    // Normally the name resolves from the store alongside the id.
    let resolved = facade
        .whoami(session)
        .effective_project
        .expect("a resolved scope");
    assert_eq!(resolved.id, record.id);
    assert_eq!(resolved.name.as_deref(), Some("storefront"));

    // Under a transient store fault the name is unreadable, but the id and the scope must survive.
    projects.set_get_failing(true);
    let dimmed = facade
        .whoami(session)
        .effective_project
        .expect("the scope is still resolved");
    assert_eq!(dimmed.id, record.id, "the resolved scope id is preserved");
    assert!(
        dimmed.name.is_none(),
        "an unreadable name dims to None, never dropping the whole scope",
    );
}
