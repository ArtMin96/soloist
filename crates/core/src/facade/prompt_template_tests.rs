use crate::facade::Facade;
use crate::ids::SessionId;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTemplateRepo, FakeTrustRepo};

/// A façade over in-memory fakes with one project loaded and the template store wired. The
/// sole loaded project gives an unbound session the single-project default scope.
fn scoped_facade() -> (Facade, SessionId) {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(
            Path::new("/tmp/soloist-prompt-template-test"),
            Some("p"),
            None,
        )
        .expect("seed one project");
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .template_repo(Arc::new(FakeTemplateRepo::new()))
        .build(),
    );
    let session = facade.open_session(None);
    (facade, session)
}

/// A façade with no projects loaded — no effective scope for an unbound session.
fn unscoped_facade() -> (Facade, SessionId) {
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .template_repo(Arc::new(FakeTemplateRepo::new()))
        .build(),
    );
    let session = facade.open_session(None);
    (facade, session)
}

#[test]
fn a_project_scoped_action_with_no_project_in_scope_is_refused() {
    let (facade, session) = unscoped_facade();

    assert!(matches!(
        facade.scoped(session).prompt_template_create(
            TemplateScope::Project,
            "review",
            None,
            "body"
        ),
        Err(CoordinationError::NoProjectScope)
    ));
    assert!(matches!(
        facade
            .scoped(session)
            .prompt_template_list(Some(TemplateScope::Project)),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn the_global_scope_needs_no_project() {
    let (facade, session) = unscoped_facade();

    let created = facade
        .scoped(session)
        .prompt_template_create(TemplateScope::Global, "review", None, "Review {{diff}}")
        .expect("a global create works with nothing in scope");

    assert_eq!(created.scope, TemplateScope::Global);
    let read = facade
        .scoped(session)
        .prompt_template_read(TemplateScope::Global, "review")
        .expect("read it back");
    assert_eq!(read.placeholders, vec!["diff".to_owned()]);
}

#[test]
fn a_scoped_session_round_trips_create_update_export_delete() {
    let (facade, session) = scoped_facade();

    let created = facade
        .scoped(session)
        .prompt_template_create(
            TemplateScope::Project,
            "review",
            Some("PR review"),
            "Review {{diff}}",
        )
        .expect("create");
    let updated = facade
        .scoped(session)
        .prompt_template_update(
            TemplateScope::Project,
            "review",
            Some("PR review"),
            "Review {{diff}} for {{focus}}",
            created.revision,
        )
        .expect("update at the read revision");
    assert_eq!(updated.revision, created.revision + 1);

    let exported = facade
        .scoped(session)
        .prompt_template_export(TemplateScope::Project, "review")
        .expect("export");
    assert_eq!(exported.body, "Review {{diff}} for {{focus}}");

    assert!(facade
        .scoped(session)
        .prompt_template_delete(TemplateScope::Project, "review")
        .expect("delete"));
    assert!(matches!(
        facade
            .scoped(session)
            .prompt_template_read(TemplateScope::Project, "review"),
        Err(CoordinationError::UnknownTemplate)
    ));
}

#[test]
fn a_stale_update_and_a_taken_name_surface_their_own_errors() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .prompt_template_create(TemplateScope::Project, "review", None, "one")
        .expect("create");

    assert!(matches!(
        facade.scoped(session).prompt_template_create(
            TemplateScope::Project,
            "review",
            None,
            "two"
        ),
        Err(CoordinationError::TemplateNameTaken)
    ));
    assert!(matches!(
        facade.scoped(session).prompt_template_update(
            TemplateScope::Project,
            "review",
            None,
            "two",
            9
        ),
        Err(CoordinationError::TemplateRevisionConflict {
            expected: Some(9),
            actual: Some(1),
        })
    ));
    assert!(matches!(
        facade
            .scoped(session)
            .prompt_template_create(TemplateScope::Project, " ", None, " "),
        Err(CoordinationError::InvalidTemplate(_))
    ));
}

#[test]
fn updating_a_missing_template_reports_it_unknown() {
    let (facade, session) = scoped_facade();

    assert!(matches!(
        facade.scoped(session).prompt_template_update(
            TemplateScope::Project,
            "ghost",
            None,
            "body",
            1
        ),
        Err(CoordinationError::UnknownTemplate)
    ));
}

#[test]
fn an_unscoped_list_merges_global_and_project_rows() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .prompt_template_create(TemplateScope::Global, "review", None, "global")
        .expect("global create");
    facade
        .scoped(session)
        .prompt_template_create(TemplateScope::Project, "triage", None, "project")
        .expect("project create");

    let merged = facade
        .scoped(session)
        .prompt_template_list(None)
        .expect("merged list");
    assert_eq!(
        merged
            .iter()
            .map(|s| (s.name.as_str(), s.scope))
            .collect::<Vec<_>>(),
        vec![
            ("review", TemplateScope::Global),
            ("triage", TemplateScope::Project)
        ]
    );

    let global_only = facade
        .scoped(session)
        .prompt_template_list(Some(TemplateScope::Global))
        .expect("global list");
    assert_eq!(global_only.len(), 1);
}

#[test]
fn an_unscoped_list_with_no_project_still_serves_the_global_rows() {
    let (facade, session) = unscoped_facade();
    facade
        .scoped(session)
        .prompt_template_create(TemplateScope::Global, "review", None, "global")
        .expect("global create");

    let merged = facade
        .scoped(session)
        .prompt_template_list(None)
        .expect("an unscoped list never fails on scope");
    assert_eq!(merged.len(), 1);
}
