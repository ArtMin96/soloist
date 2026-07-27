//! The seeding seam and the default-template selection, exercised end to end through the façade.

use crate::PeerCredentials;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{RenderError, RenderRequest, TodoDoc};
use crate::ids::{ProjectId, SessionId, TemplateId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::template::TemplateKind;
use crate::testing::{
    FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTemplateRepo, FakeTrustRepo,
};
use crate::TodoStatus;

/// A façade over in-memory fakes with one project loaded, a template store, and both settings
/// stores — everything the seeding seam reads. The sole loaded project gives an unbound session the
/// single-project default scope, and is returned so a test can select that project's default.
fn facade() -> (Facade, SessionId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(
            Path::new("/tmp/soloist-template-seed-test"),
            Some("p"),
            None,
        )
        .expect("seed one project")
        .id;
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
    (facade, session, project)
}

/// A façade over the same fakes with **two** distinct projects loaded, returned with both ids —
/// the only way to observe that one project's template library is not another's.
fn facade_with_two_projects() -> (Facade, ProjectId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let first = projects
        .upsert(Path::new("/tmp/soloist-template-scope-a"), Some("a"), None)
        .expect("seed the first project")
        .id;
    let second = projects
        .upsert(Path::new("/tmp/soloist-template-scope-b"), Some("b"), None)
        .expect("seed the second project")
        .id;
    assert_ne!(first, second, "the two projects must be distinct");
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
    (facade, first, second)
}

/// The names the manager lists for `kind` in `project`'s scope.
fn listed_names(facade: &Facade, kind: TemplateKind, project: Option<ProjectId>) -> Vec<String> {
    facade
        .templates(kind, project)
        .expect("list the scope")
        .into_iter()
        .map(|summary| summary.name)
        .collect()
}

/// Seeds a template of `kind` in `project`'s own library and selects it as that project's default,
/// returning its id.
fn default_template(
    facade: &Facade,
    kind: TemplateKind,
    project: ProjectId,
    name: &str,
    body: &str,
) -> TemplateId {
    let created = facade
        .templates
        .create(kind, Some(project), name, None, body)
        .expect("create the template");
    facade
        .set_default_template(kind, project, Some(created.id))
        .expect("select the default");
    created.id
}

#[test]
fn creating_an_empty_scratchpad_seeds_the_default_template_body() {
    let (facade, session, project) = facade();
    default_template(
        &facade,
        TemplateKind::Scratchpad,
        project,
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
    let (facade, session, project) = facade();
    default_template(
        &facade,
        TemplateKind::Todo,
        project,
        "chore",
        "## Steps\n\n- [ ] do it",
    );

    let created = facade
        .scoped(session)
        .todo_create(
            TodoDoc {
                title: "sweep".into(),
                body: String::new(),
                status: TodoStatus::Open,
            },
            None,
        )
        .expect("create");

    assert_eq!(created.view.doc.body, "## Steps\n\n- [ ] do it");
    assert_eq!(created.seeded_from.as_deref(), Some("chore"));
}

#[test]
fn with_no_default_selected_an_empty_creation_stays_empty() {
    let (facade, session, _project) = facade();

    let written = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("write");

    assert_eq!(written.view.body, "", "a blank document is valid");
    assert_eq!(written.seeded_from, None);
}

#[test]
fn a_nonempty_body_is_written_verbatim_and_never_seeded() {
    let (facade, session, project) = facade();
    default_template(
        &facade,
        TemplateKind::Scratchpad,
        project,
        "daily",
        "seed body",
    );

    let written = facade
        .scoped(session)
        .scratchpad_write("today", "my own body".into(), None)
        .expect("write");

    assert_eq!(written.view.body, "my own body");
    assert_eq!(written.seeded_from, None);
}

#[test]
fn an_update_is_never_seeded_even_with_an_empty_body() {
    let (facade, session, project) = facade();
    default_template(
        &facade,
        TemplateKind::Scratchpad,
        project,
        "daily",
        "seed body",
    );
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
    let (facade, session, project) = facade();
    default_template(
        &facade,
        TemplateKind::Scratchpad,
        project,
        "daily",
        "seed body",
    );
    // The selected default is deleted after selection — a stale id resolves to nothing.
    assert!(facade
        .templates
        .delete(TemplateKind::Scratchpad, Some(project), "daily")
        .expect("delete"));

    let written = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("write");

    assert_eq!(written.view.body, "");
    assert_eq!(written.seeded_from, None);
}

#[test]
fn a_projects_default_seeds_only_that_projects_creates() {
    let (facade, first, second) = facade_with_two_projects();
    default_template(
        &facade,
        TemplateKind::Scratchpad,
        first,
        "daily",
        "the first project's shape",
    );

    let seeded = facade
        .write_scratchpad(first, "today", String::new(), None)
        .expect("write in the selecting project");
    assert_eq!(seeded.view.body, "the first project's shape");
    assert_eq!(seeded.seeded_from.as_deref(), Some("daily"));

    // A selection belongs to the project that made it: the sibling chose nothing, so its create
    // starts blank rather than inheriting a shape from a project it has nothing to do with.
    let sibling = facade
        .write_scratchpad(second, "today", String::new(), None)
        .expect("write in the sibling project");
    assert_eq!(sibling.view.body, "");
    assert_eq!(sibling.seeded_from, None);
}

#[test]
fn a_global_template_is_never_seeded_into_a_project() {
    let (facade, session, project) = facade();
    let global = facade
        .template_create(TemplateKind::Scratchpad, None, "house", None, "global body")
        .expect("author a global template");

    // Nothing falls back to the global library: with no selection an empty create stays empty even
    // though a template of that kind exists there.
    let unselected = facade
        .scoped(session)
        .scratchpad_write("today", String::new(), None)
        .expect("write");
    assert_eq!(unselected.view.body, "");
    assert_eq!(unselected.seeded_from, None);

    // Nor can a global template become a project's default: the selection is resolved in the
    // project's own library, which does not hold that id.
    facade
        .set_default_template(TemplateKind::Scratchpad, project, Some(global.id))
        .expect("point the selection at the global template");
    let selected = facade
        .scoped(session)
        .scratchpad_write("tomorrow", String::new(), None)
        .expect("write");
    assert_eq!(selected.view.body, "");
    assert_eq!(selected.seeded_from, None);
}

#[test]
fn the_manager_creates_reads_lists_and_updates_a_global_template() {
    let (facade, _session, _project) = facade();

    let created = facade
        .template_create(
            TemplateKind::Scratchpad,
            None,
            "daily",
            Some("a daily note"),
            "## Plan",
        )
        .expect("create");
    assert_eq!(created.scope, crate::template::TemplateScope::Global);

    // The listing surfaces the new template for its kind, and reading returns the full body.
    let listed = facade
        .templates(TemplateKind::Scratchpad, None)
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "daily");
    let read = facade
        .template_read(TemplateKind::Scratchpad, None, "daily")
        .expect("read");
    assert_eq!(read.body, "## Plan");

    // A revision-guarded update replaces the body and bumps the revision.
    let updated = facade
        .template_update(
            TemplateKind::Scratchpad,
            None,
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
    let (facade, _session, _project) = facade();
    let created = facade
        .template_create(TemplateKind::Todo, None, "chore", None, "body")
        .expect("create");

    assert!(matches!(
        facade.template_create(TemplateKind::Todo, None, "chore", None, "other"),
        Err(CoordinationError::TemplateNameTaken)
    ));
    assert!(matches!(
        facade.template_update(
            TemplateKind::Todo,
            None,
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
    let (facade, _session, project) = facade();
    let created = facade
        .template_create(
            TemplateKind::Scratchpad,
            Some(project),
            "daily",
            None,
            "seed body",
        )
        .expect("create");
    facade
        .set_default_template(TemplateKind::Scratchpad, project, Some(created.id))
        .expect("select the default");

    // Deleting the selected default through the manager path clears the dangling selection in core,
    // so the settings surface reflects the removal at once (not just at resolve time).
    assert!(facade
        .template_delete(TemplateKind::Scratchpad, Some(project), "daily")
        .expect("delete"));
    assert_eq!(
        facade
            .template_defaults(project)
            .expect("read defaults")
            .get(TemplateKind::Scratchpad),
        None,
    );
    assert!(facade
        .templates(TemplateKind::Scratchpad, Some(project))
        .expect("list")
        .is_empty());
}

#[test]
fn deleting_a_non_default_template_leaves_another_kinds_default_untouched() {
    let (facade, _session, project) = facade();
    let scratch_default = default_template(
        &facade,
        TemplateKind::Scratchpad,
        project,
        "daily",
        "seed body",
    );
    facade
        .template_create(
            TemplateKind::Todo,
            Some(project),
            "chore",
            None,
            "chore body",
        )
        .expect("create a todo template");

    // Deleting the todo template must not clear the unrelated scratchpad default.
    assert!(facade
        .template_delete(TemplateKind::Todo, Some(project), "chore")
        .expect("delete"));
    assert_eq!(
        facade
            .template_defaults(project)
            .expect("read defaults")
            .get(TemplateKind::Scratchpad),
        Some(scratch_default),
    );
}

#[test]
fn the_default_selection_round_trips_and_prompt_has_no_seed_default() {
    let (facade, _session, project) = facade();
    let id = TemplateId::from_raw(7);

    let defaults = facade
        .set_default_template(TemplateKind::Todo, project, Some(id))
        .expect("set todo default");
    assert_eq!(defaults.todo, Some(id));
    assert_eq!(defaults.scratchpad, None);

    // A prompt has no seed default — the setter is a no-op, so nothing is stored for it.
    let after_prompt = facade
        .set_default_template(TemplateKind::Prompt, project, Some(TemplateId::from_raw(9)))
        .expect("set prompt default");
    assert_eq!(after_prompt.get(TemplateKind::Prompt), None);
    assert_eq!(after_prompt.todo, Some(id), "the todo default is untouched");

    // The read-per-call getter reflects the persisted selection.
    assert_eq!(
        facade
            .template_defaults(project)
            .expect("read defaults")
            .get(TemplateKind::Todo),
        Some(id)
    );
}

#[test]
fn the_local_render_fills_a_template_at_the_scope_it_names() {
    let (facade, _session, _project) = facade();
    facade
        .template_create(
            TemplateKind::Prompt,
            None,
            "review",
            None,
            "Review {{diff}} for {{concern}}",
        )
        .expect("create a global prompt template");

    let request = RenderRequest {
        name: "review".to_owned(),
        values: [("diff".to_owned(), "the patch".to_owned())]
            .into_iter()
            .collect(),
        ..RenderRequest::default()
    };

    let rendered = facade
        .template_render(None, &request)
        .expect("render the global template");
    assert_eq!(rendered.text, "Review the patch for {{concern}}");
    assert_eq!(rendered.unfilled, vec!["concern".to_owned()]);

    // The scope is the caller's to state: the same name is absent from a project's library.
    let refused = facade.template_render(Some(ProjectId::from_raw(1)), &request);
    assert!(
        matches!(refused, Err(RenderError::TemplateNotFound)),
        "expected the project scope to be empty, got {refused:?}"
    );
}

#[test]
fn the_manager_addresses_each_project_scope_separately() {
    let (facade, first, second) = facade_with_two_projects();

    facade
        .template_create(TemplateKind::Prompt, Some(first), "notes", None, "body")
        .expect("create in the first project");

    // The template belongs to the scope it was written to and to no other — not the sibling
    // project's library, and not the global one the manager used to be hard-wired to.
    assert_eq!(
        listed_names(&facade, TemplateKind::Prompt, Some(first)),
        vec!["notes".to_owned()]
    );
    assert!(listed_names(&facade, TemplateKind::Prompt, Some(second)).is_empty());
    assert!(listed_names(&facade, TemplateKind::Prompt, None).is_empty());

    // Reading and deleting address the same scope: the sibling project has nothing under the name.
    assert!(matches!(
        facade.template_read(TemplateKind::Prompt, Some(second), "notes"),
        Err(CoordinationError::UnknownTemplate)
    ));
    assert!(!facade
        .template_delete(TemplateKind::Prompt, Some(second), "notes")
        .expect("delete from the sibling project"));
    assert_eq!(
        listed_names(&facade, TemplateKind::Prompt, Some(first)),
        vec!["notes".to_owned()],
        "a delete aimed at another scope must not remove this one's template"
    );
}

#[test]
fn one_name_holds_a_separate_template_in_each_scope() {
    let (facade, first, second) = facade_with_two_projects();
    for (project, body) in [
        (None, "global body"),
        (Some(first), "first body"),
        (Some(second), "second body"),
    ] {
        facade
            .template_create(TemplateKind::Todo, project, "chore", None, body)
            .expect("create the scope's template");
    }

    let created = facade
        .template_read(TemplateKind::Todo, Some(first), "chore")
        .expect("read the first project's");
    facade
        .template_update(
            TemplateKind::Todo,
            Some(first),
            "chore",
            None,
            "edited body",
            created.revision,
        )
        .expect("update the first project's");

    // Editing one scope's template leaves the identically-named ones in the other scopes alone.
    let body_in = |project| {
        facade
            .template_read(TemplateKind::Todo, project, "chore")
            .expect("read the scope's template")
            .body
    };
    assert_eq!(body_in(Some(first)), "edited body");
    assert_eq!(body_in(Some(second)), "second body");
    assert_eq!(body_in(None), "global body");
}

/// Every template change a listener was told about, as the `(kind, scope)` pair it named.
fn announced(
    events: &mut tokio::sync::broadcast::Receiver<DomainEvent>,
) -> Vec<(TemplateKind, Option<ProjectId>)> {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            DomainEvent::TemplateChanged { kind, project } => Some((kind, project)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_manager_write_announces_the_scope_it_changed() {
    let (facade, first, _second) = facade_with_two_projects();
    let mut events = facade.subscribe();

    let created = facade
        .template_create(TemplateKind::Scratchpad, None, "daily", None, "body")
        .expect("create globally");
    facade
        .template_create(
            TemplateKind::Scratchpad,
            Some(first),
            "sprint",
            None,
            "body",
        )
        .expect("create in the project");
    facade
        .template_update(
            TemplateKind::Scratchpad,
            None,
            "daily",
            None,
            "edited",
            created.revision,
        )
        .expect("update the global one");
    assert!(facade
        .template_delete(TemplateKind::Scratchpad, Some(first), "sprint")
        .expect("delete from the project"));

    // A listener re-reads the list the event names, so both halves must ride along: a project write
    // announced as global would send it back to a library that did not change, and an edit that
    // announced nothing at all would leave the panel showing the pre-edit body.
    assert_eq!(
        announced(&mut events),
        vec![
            (TemplateKind::Scratchpad, None),
            (TemplateKind::Scratchpad, Some(first)),
            (TemplateKind::Scratchpad, None),
            (TemplateKind::Scratchpad, Some(first)),
        ]
    );
}

#[test]
fn a_delete_that_removed_nothing_announces_nothing() {
    let (facade, first, second) = facade_with_two_projects();
    facade
        .template_create(TemplateKind::Todo, Some(first), "chore", None, "body")
        .expect("create in the first project");
    let mut events = facade.subscribe();

    // Neither delete finds a row: one aims at a name no scope holds, the other at a scope that
    // never held this name. A listener woken by either would re-read a library that did not move.
    assert!(!facade
        .template_delete(TemplateKind::Todo, Some(first), "ghost")
        .expect("delete an absent name"));
    assert!(!facade
        .template_delete(TemplateKind::Todo, Some(second), "chore")
        .expect("delete from the sibling project"));

    assert_eq!(announced(&mut events), Vec::new());
}
