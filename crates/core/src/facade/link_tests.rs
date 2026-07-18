use crate::facade::Facade;
use crate::ids::SessionId;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{Link, LinkContent, TodoDoc, TodoStatus};
use crate::ids::{ProjectId, ScratchpadId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    FakeProjectRepo, FakeScratchpadRepo, FakeSpawner, FakeTodoRepo, FakeTrustRepo,
};

fn scratchpad_body() -> String {
    "## Objective\nShip v1\n\n## Status\nin progress".to_owned()
}

fn todo_doc() -> TodoDoc {
    TodoDoc {
        title: "wire it".into(),
        body: "do it".into(),
        status: TodoStatus::Open,
    }
}

/// A façade with one project loaded and both the scratchpad and todo stores wired, plus its
/// single-project effective scope on an unbound session. Returns the project id so a test can build
/// links for it (and for a neighbouring, out-of-scope project).
fn facade() -> (Facade, SessionId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(Path::new("/tmp/soloist-link-test"), Some("p"), None)
        .expect("seed one project")
        .id;
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .scratchpad_repo(Arc::new(FakeScratchpadRepo::new()))
        .todo_repo(Arc::new(FakeTodoRepo::new()))
        .build(),
    );
    let session = facade.open_session(None);
    (facade, session, project)
}

#[test]
fn a_scratchpad_link_resolves_within_scope() {
    let (facade, session, project) = facade();
    let pad = facade
        .scoped(session)
        .scratchpad_write("release-plan", scratchpad_body(), None)
        .expect("create")
        .view;
    let link = Link::scratchpad(project, pad.id).to_link();

    let content = facade
        .scoped(session)
        .resolve_link(&link)
        .expect("resolves in scope");
    assert_eq!(content, LinkContent::Scratchpad(pad));
}

#[test]
fn a_todo_link_resolves_within_scope() {
    let (facade, session, project) = facade();
    let todo = facade
        .scoped(session)
        .todo_create(todo_doc())
        .expect("create")
        .view;
    let link = Link::todo(project, todo.id).to_link();

    let content = facade
        .scoped(session)
        .resolve_link(&link)
        .expect("resolves in scope");
    assert_eq!(content, LinkContent::Todo(todo));
}

#[test]
fn a_foreign_scope_link_is_refused_not_resolved() {
    let (facade, session, project) = facade();
    let pad = facade
        .scoped(session)
        .scratchpad_write("release-plan", scratchpad_body(), None)
        .expect("create")
        .view;
    // The same id but a different project must be refused, never resolved to our content.
    let foreign = Link::scratchpad(ProjectId::from_raw(project.get() + 1), pad.id).to_link();

    assert!(matches!(
        facade.scoped(session).resolve_link(&foreign),
        Err(CoordinationError::ForeignScopeLink)
    ));
}

#[test]
fn a_malformed_link_is_refused() {
    let (facade, session, _) = facade();
    assert!(matches!(
        facade.scoped(session).resolve_link("not-a-link"),
        Err(CoordinationError::MalformedLink)
    ));
}

#[test]
fn an_unknown_in_scope_target_is_reported() {
    let (facade, session, project) = facade();
    let link = Link::scratchpad(project, ScratchpadId::from_raw(999)).to_link();

    assert!(matches!(
        facade.scoped(session).resolve_link(&link),
        Err(CoordinationError::UnknownScratchpad)
    ));
}
