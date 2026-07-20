use super::*;

use soloist_core::composition::CorePorts;
use soloist_core::ports::TokioClock;
use soloist_core::testing::{FakeProjectRepo, FakeSpawner, FakeTemplateRepo, FakeTrustRepo};
use soloist_core::TemplateKind;

/// A façade over in-memory fakes with a template library, for the preview render.
fn facade() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .template_repo(Arc::new(FakeTemplateRepo::new()))
        .build(),
    )
}

#[test]
fn the_preview_renders_a_half_filled_template_instead_of_refusing_it() {
    let facade = facade();
    facade
        .template_create(
            TemplateKind::Prompt,
            None,
            "review",
            None,
            "Review {{diff}} for {{concern}}",
        )
        .expect("create a global prompt template");

    let request = preview_request(
        "review".to_owned(),
        BTreeMap::from([("diff".to_owned(), "the patch".to_owned())]),
    );
    let rendered = facade
        .template_render(None, &request)
        .expect("the preview renders with a value still missing");

    // The pane has something to say the moment a template is opened, before anything is typed: the
    // unfilled marker survives in the text and is named alongside it. Under the strict policy this
    // same render is refused outright and the preview goes blank.
    assert_eq!(rendered.text, "Review the patch for {{concern}}");
    assert_eq!(rendered.unfilled, vec!["concern".to_owned()]);
}
