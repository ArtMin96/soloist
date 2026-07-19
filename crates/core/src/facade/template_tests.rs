//! The seeding seam and the default-template selection, exercised end to end through the façade.

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::TodoDoc;
use crate::ids::{SessionId, TemplateId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::template::TemplateKind;
use crate::testing::{
    FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTemplateRepo, FakeTrustRepo,
};
use crate::TodoStatus;

/// A façade over in-memory fakes with one project loaded, a template store, and a settings store —
/// everything the seeding seam reads. The sole loaded project gives an unbound session the
/// single-project default scope.
fn facade() -> (Facade, SessionId) {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(
            Path::new("/tmp/soloist-template-seed-test"),
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
        .settings_repo(Arc::new(FakeSettingsRepo::new()))
        .build(),
    );
    let session = facade.open_session(None);
    (facade, session)
}

/// Seeds a global template of `kind` and selects it as the default, returning its id.
fn default_template(facade: &Facade, kind: TemplateKind, name: &str, body: &str) -> TemplateId {
    let created = facade
        .templates
        .create(kind, None, name, None, body)
        .expect("create the template");
    facade
        .set_default_template(kind, Some(created.id))
        .expect("select the default");
    created.id
}

#[test]
fn creating_an_empty_scratchpad_seeds_the_default_template_body() {
    let (facade, session) = facade();
    default_template(
        &facade,
        TemplateKind::Scratchpad,
        "daily",
        "## Plan\n\n- [ ] first",
    );

    let written = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("write");

    assert_eq!(written.view.body, "## Plan\n\n- [ ] first");
    assert_eq!(
        written.seeded_from.as_deref(),
        Some("daily"),
        "the create response names the seeding template"
    );
}

#[test]
fn creating_an_empty_todo_seeds_the_default_template_body() {
    let (facade, session) = facade();
    default_template(
        &facade,
        TemplateKind::Todo,
        "chore",
        "## Steps\n\n- [ ] do it",
    );

    let created = facade
        .scoped(session)
        .todo_create(TodoDoc {
            title: "sweep".into(),
            body: String::new(),
            status: TodoStatus::Open,
        })
        .expect("create");

    assert_eq!(created.view.doc.body, "## Steps\n\n- [ ] do it");
    assert_eq!(created.seeded_from.as_deref(), Some("chore"));
}

#[test]
fn with_no_default_selected_an_empty_creation_stays_empty() {
    let (facade, session) = facade();

    let written = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("write");

    assert_eq!(written.view.body, "", "a blank document is valid");
    assert_eq!(written.seeded_from, None);
}

#[test]
fn a_nonempty_body_is_written_verbatim_and_never_seeded() {
    let (facade, session) = facade();
    default_template(&facade, TemplateKind::Scratchpad, "daily", "seed body");

    let written = facade
        .scoped(session)
        .scratchpad_write("today", "my own body".into(), None)
        .expect("write");

    assert_eq!(written.view.body, "my own body");
    assert_eq!(written.seeded_from, None);
}

#[test]
fn an_update_is_never_seeded_even_with_an_empty_body() {
    let (facade, session) = facade();
    default_template(&facade, TemplateKind::Scratchpad, "daily", "seed body");
    let created = facade
        .scoped(session)
        .scratchpad_write("today", "original".into(), None)
        .expect("create");

    // A revision-guarded update (expected is Some) clears the body — it must not re-seed.
    let updated = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), Some(created.view.revision))
        .expect("update");

    assert_eq!(updated.view.body, "");
    assert_eq!(updated.seeded_from, None);
}

#[test]
fn a_deleted_default_template_falls_back_to_an_empty_body() {
    let (facade, session) = facade();
    default_template(&facade, TemplateKind::Scratchpad, "daily", "seed body");
    // The selected default is deleted after selection — a stale id resolves to nothing.
    assert!(facade
        .templates
        .delete(TemplateKind::Scratchpad, None, "daily")
        .expect("delete"));

    let written = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("write");

    assert_eq!(written.view.body, "");
    assert_eq!(written.seeded_from, None);
}

#[test]
fn the_manager_creates_reads_lists_and_updates_a_global_template() {
    let (facade, _session) = facade();

    let created = facade
        .template_create(
            TemplateKind::Scratchpad,
            "daily",
            Some("a daily note"),
            "## Plan",
        )
        .expect("create");
    assert_eq!(created.scope, crate::template::TemplateScope::Global);

    // The listing surfaces the new template for its kind, and reading returns the full body.
    let listed = facade.templates(TemplateKind::Scratchpad).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "daily");
    let read = facade
        .template_read(TemplateKind::Scratchpad, "daily")
        .expect("read");
    assert_eq!(read.body, "## Plan");

    // A revision-guarded update replaces the body and bumps the revision.
    let updated = facade
        .template_update(
            TemplateKind::Scratchpad,
            "daily",
            Some("a daily note"),
            "## Plan\n\n- [ ] first",
            created.revision,
        )
        .expect("update");
    assert_eq!(updated.body, "## Plan\n\n- [ ] first");
    assert!(updated.revision > created.revision);
}

#[test]
fn a_taken_template_name_and_a_stale_update_are_refused() {
    let (facade, _session) = facade();
    let created = facade
        .template_create(TemplateKind::Todo, "chore", None, "body")
        .expect("create");

    assert!(matches!(
        facade.template_create(TemplateKind::Todo, "chore", None, "other"),
        Err(CoordinationError::TemplateNameTaken)
    ));
    assert!(matches!(
        facade.template_update(
            TemplateKind::Todo,
            "chore",
            None,
            "body2",
            created.revision + 9
        ),
        Err(CoordinationError::TemplateRevisionConflict { .. })
    ));
}

#[test]
fn deleting_a_template_that_is_the_selected_default_clears_the_selection() {
    let (facade, _session) = facade();
    let created = facade
        .template_create(TemplateKind::Scratchpad, "daily", None, "seed body")
        .expect("create");
    facade
        .set_default_template(TemplateKind::Scratchpad, Some(created.id))
        .expect("select the default");

    // Deleting the selected default through the manager path clears the dangling selection in core,
    // so the settings surface reflects the removal at once (not just at resolve time).
    assert!(facade
        .template_delete(TemplateKind::Scratchpad, "daily")
        .expect("delete"));
    assert_eq!(
        facade
            .template_defaults()
            .expect("read defaults")
            .get(TemplateKind::Scratchpad),
        None,
    );
    assert!(facade
        .templates(TemplateKind::Scratchpad)
        .expect("list")
        .is_empty());
}

#[test]
fn deleting_a_non_default_template_leaves_another_kinds_default_untouched() {
    let (facade, _session) = facade();
    let scratch_default = default_template(&facade, TemplateKind::Scratchpad, "daily", "seed body");
    facade
        .template_create(TemplateKind::Todo, "chore", None, "chore body")
        .expect("create a todo template");

    // Deleting the todo template must not clear the unrelated scratchpad default.
    assert!(facade
        .template_delete(TemplateKind::Todo, "chore")
        .expect("delete"));
    assert_eq!(
        facade
            .template_defaults()
            .expect("read defaults")
            .get(TemplateKind::Scratchpad),
        Some(scratch_default),
    );
}

#[test]
fn the_default_selection_round_trips_and_prompt_has_no_seed_default() {
    let (facade, _session) = facade();
    let id = TemplateId::from_raw(7);

    let defaults = facade
        .set_default_template(TemplateKind::Todo, Some(id))
        .expect("set todo default");
    assert_eq!(defaults.todo, Some(id));
    assert_eq!(defaults.scratchpad, None);

    // A prompt has no seed default — the setter is a no-op, so nothing is stored for it.
    let after_prompt = facade
        .set_default_template(TemplateKind::Prompt, Some(TemplateId::from_raw(9)))
        .expect("set prompt default");
    assert_eq!(after_prompt.get(TemplateKind::Prompt), None);
    assert_eq!(after_prompt.todo, Some(id), "the todo default is untouched");

    // The read-per-call getter reflects the persisted selection.
    assert_eq!(
        facade
            .template_defaults()
            .expect("read defaults")
            .get(TemplateKind::Todo),
        Some(id)
    );
}
