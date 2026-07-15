use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{Link, LinkContent, ScratchpadDoc, TodoDoc, TodoStatus};
use crate::ids::{ProjectId, ScratchpadId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{
    FakeProjectRepo, FakeScratchpadRepo, FakeSpawner, FakeTodoRepo, FakeTrustRepo,
};

fn scratchpad_doc() -> ScratchpadDoc {
    ScratchpadDoc {
        objective: "Ship v1".into(),
        context: "RC cut".into(),
        plan: vec!["Cut RC".into()],
        acceptance_criteria: vec!["soak green".into()],
        risks: vec!["none identified".into()],
        status: "in progress".into(),
        notes: None,
    }
}

fn todo_doc() -> TodoDoc {
    TodoDoc {
        title: "wire it".into(),
        description: "do it".into(),
        acceptance_criteria: vec!["done".into()],
        risks: vec!["none identified".into()],
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
        .scratchpad_write(session, "release-plan", scratchpad_doc(), None)
        .expect("create");
    let link = Link::scratchpad(project, pad.id).to_link();

    let content = facade
        .resolve_link(session, &link)
        .expect("resolves in scope");
    assert_eq!(content, LinkContent::Scratchpad(pad));
}

#[test]
fn a_todo_link_resolves_within_scope() {
    let (facade, session, project) = facade();
    let todo = facade.todo_create(session, todo_doc()).expect("create");
    let link = Link::todo(project, todo.id).to_link();

    let content = facade
        .resolve_link(session, &link)
        .expect("resolves in scope");
    assert_eq!(content, LinkContent::Todo(todo));
}

#[test]
fn a_foreign_scope_link_is_refused_not_resolved() {
    let (facade, session, project) = facade();
    let pad = facade
        .scratchpad_write(session, "release-plan", scratchpad_doc(), None)
        .expect("create");
    // The same id but a different project must be refused, never resolved to our content.
    let foreign = Link::scratchpad(ProjectId::from_raw(project.get() + 1), pad.id).to_link();

    assert!(matches!(
        facade.resolve_link(session, &foreign),
        Err(CoordinationError::ForeignScopeLink)
    ));
}

#[test]
fn a_malformed_link_is_refused() {
    let (facade, session, _) = facade();
    assert!(matches!(
        facade.resolve_link(session, "not-a-link"),
        Err(CoordinationError::MalformedLink)
    ));
}

#[test]
fn an_unknown_in_scope_target_is_reported() {
    let (facade, session, project) = facade();
    let link = Link::scratchpad(project, ScratchpadId::from_raw(999)).to_link();

    assert!(matches!(
        facade.resolve_link(session, &link),
        Err(CoordinationError::UnknownScratchpad)
    ));
}
