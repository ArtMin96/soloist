//! The session-scoped seed-template peek: what it shows, where it is refused, and the write
//! boundary it does not cross.

use crate::PeerCredentials;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::ids::{ProjectId, SessionId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::template::TemplateScope;
use crate::testing::{
    FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTemplateRepo, FakeTrustRepo,
};

/// A façade over in-memory fakes with one project loaded, a template store, and both settings
/// stores — everything the seeding seam reads. The sole loaded project gives an unbound session the
/// single-project default scope, and is returned so a test can select that project's default.
fn facade() -> (Facade, SessionId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(
            Path::new("/tmp/soloist-seed-template-test"),
            Some("p"),
            None,
        )
        .expect("seed one project")
        .id;
    let (facade, session) = facade_over(projects);
    (facade, session, project)
}

/// A façade with no projects loaded — an unbound session has no effective scope.
fn unscoped_facade() -> (Facade, SessionId) {
    facade_over(Arc::new(FakeProjectRepo::new()))
}

fn facade_over(projects: Arc<FakeProjectRepo>) -> (Facade, SessionId) {
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .template_repo(Arc::new(FakeTemplateRepo::new()))
        .settings_repo(Arc::new(FakeSettingsRepo::new()))
        .project_settings_repo(Arc::new(FakeSettingsRepo::new()))
        .build(),
    );
    let session = facade.open_session(PeerCredentials::unauthenticated());
    (facade, session)
}

/// The user authors a template of `kind` in `project`'s library and selects it as that project's
/// default.
fn selected_default(
    facade: &Facade,
    kind: TemplateKind,
    project: ProjectId,
    name: &str,
    body: &str,
) {
    let created = facade
        .template_create(kind, Some(project), name, None, body)
        .expect("author the template");
    facade
        .set_default_template(kind, project, Some(created.id))
        .expect("select it as the default");
}

#[test]
fn the_peek_shows_the_template_a_create_would_seed_from() {
    let (facade, session, project) = facade();
    selected_default(
        &facade,
        TemplateKind::Scratchpad,
        project,
        "daily",
        "## Plan\n\n- [ ] first",
    );

    let peeked = facade
        .scoped(session)
        .seed_template(TemplateKind::Scratchpad)
        .expect("peek")
        .expect("a default is selected");

    // A shape a create would not actually apply is worse than none, so the peek is checked against
    // a real create rather than against the template it was read from.
    let seeded = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("create an empty scratchpad");

    assert_eq!(peeked.name, "daily");
    assert_eq!(peeked.body, seeded.view.body);
    assert_eq!(seeded.seeded_from, Some(peeked.name));
}

#[test]
fn a_template_the_user_authored_but_did_not_select_is_not_shown() {
    let (facade, session, project) = facade();
    facade
        .template_create(
            TemplateKind::Scratchpad,
            Some(project),
            "draft",
            None,
            "not the house style",
        )
        .expect("author a template without selecting it");

    // The peek answers "what would seed a create", not "what is in the user's library" — an
    // unselected template seeds nothing and so is not the caller's to see.
    assert!(facade
        .scoped(session)
        .seed_template(TemplateKind::Scratchpad)
        .expect("peek")
        .is_none());
}

#[test]
fn a_peek_with_no_project_in_scope_is_refused() {
    let (facade, session) = unscoped_facade();

    assert!(matches!(
        facade
            .scoped(session)
            .seed_template(TemplateKind::Scratchpad),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn a_scoped_caller_cannot_write_a_seed_kind_template() {
    let (facade, session, _project) = facade();
    facade
        .template_create(
            TemplateKind::Scratchpad,
            None,
            "house-style",
            None,
            "## Plan",
        )
        .expect("the user authors the scratchpad template");

    // The only template write a session-scoped caller has addresses the prompt library, so the
    // same name there is a different template and the scratchpad one is untouched.
    facade
        .scoped(session)
        .prompt_template_create(TemplateScope::Global, "house-style", None, "hijacked")
        .expect("the prompt library has no template of that name, so this creates one");

    assert_eq!(
        facade
            .template_read(TemplateKind::Scratchpad, None, "house-style")
            .expect("the scratchpad template still exists")
            .body,
        "## Plan",
    );
}

#[test]
fn a_scoped_caller_cannot_delete_a_seed_kind_template() {
    let (facade, session, _project) = facade();
    facade
        .template_create(
            TemplateKind::Scratchpad,
            None,
            "house-style",
            None,
            "## Plan",
        )
        .expect("the user authors the scratchpad template");

    assert!(
        !facade
            .scoped(session)
            .prompt_template_delete(TemplateScope::Global, "house-style")
            .expect("the delete is answered, not refused"),
        "the name exists only in the scratchpad library, which this caller cannot address",
    );
    assert!(facade
        .template_read(TemplateKind::Scratchpad, None, "house-style")
        .is_ok());
}
