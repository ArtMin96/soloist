use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{session_in_dir, FakeProjectRepo, FakeSpawner, FakeTrustRepo};

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
    let session = facade.open_session(PeerCredentials::unauthenticated());

    // Normally the name resolves from the store alongside the id.
    let resolved = facade
        .scoped(session)
        .whoami()
        .effective_project
        .expect("a resolved scope");
    assert_eq!(resolved.id, record.id);
    assert_eq!(resolved.name.as_deref(), Some("storefront"));

    // Under a transient store fault the name is unreadable, but the id and the scope must survive.
    projects.set_get_failing(true);
    let dimmed = facade
        .scoped(session)
        .whoami()
        .effective_project
        .expect("the scope is still resolved");
    assert_eq!(dimmed.id, record.id, "the resolved scope id is preserved");
    assert!(
        dimmed.name.is_none(),
        "an unreadable name dims to None, never dropping the whole scope",
    );
}

/// An agent Soloist did not launch — no managed process in its group — still resolves its scope
/// from the directory it runs in, even with several projects open (so the sole-project default does
/// not apply). This is the whole point of the directory signal: the agent knows its project without
/// selecting anything, and can select only the project it runs in.
#[test]
fn an_external_agents_working_directory_scopes_it_to_the_project_it_runs_in() {
    let projects = Arc::new(FakeProjectRepo::new());
    let soloist = projects
        .upsert(Path::new("/home/dev/soloist"), Some("Soloist"), None)
        .expect("seed soloist");
    let trackler = projects
        .upsert(Path::new("/home/dev/trackler"), Some("trackler"), None)
        .expect("seed trackler");
    let _other = projects
        .upsert(Path::new("/home/dev/other"), Some("other"), None)
        .expect("seed other");
    let facade = facade_over(projects.clone());

    // The peer's working directory is inside the Soloist project's root.
    let session = session_in_dir(&facade, PathBuf::from("/home/dev/soloist/crates/core"));

    // Its effective scope is the Soloist project — resolved with no `select_project` call.
    assert_eq!(facade.effective_project(session), Some(soloist.id));
    assert_eq!(
        facade
            .scoped(session)
            .whoami()
            .effective_project
            .map(|project| project.id),
        Some(soloist.id),
        "whoami reports the directory-resolved project without the agent selecting anything",
    );

    // It may explicitly select the project it runs in...
    assert!(facade.scoped(session).select_project(soloist.id).is_ok());
    // ...but never a sibling it does not run in — the cross-project isolation still holds.
    assert!(
        matches!(
            facade.scoped(session).select_project(trackler.id),
            Err(IdentityError::ForeignProject),
        ),
        "selecting a project the agent does not run in is still refused",
    );
}

/// With several projects open and a working directory inside none of them, the directory signal
/// resolves nothing and the sole-project default does not apply, so the session has no effective
/// project — a scoped tool must ask it to select one.
#[test]
fn a_working_directory_outside_every_project_grants_no_scope() {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/home/dev/soloist"), Some("Soloist"), None)
        .expect("seed soloist");
    projects
        .upsert(Path::new("/home/dev/trackler"), Some("trackler"), None)
        .expect("seed trackler");
    let facade = facade_over(projects.clone());

    let session = session_in_dir(&facade, PathBuf::from("/home/dev/unrelated"));
    assert_eq!(facade.effective_project(session), None);
}
