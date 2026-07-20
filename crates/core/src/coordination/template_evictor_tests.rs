//! Behavioural tests for [`TemplateEvictor`]: a removed project's cached template rows are dropped,
//! and only that project's. They drive a real [`Templates`] over the counting fake repo and delete
//! rows through the repo directly — which is what removing a project does, the store cascading them
//! away with no write the aggregate can see — so the cache is left holding rows the store no longer
//! has, exactly as in the app.

use std::sync::Arc;

use super::super::template_repo::TemplateRepo;
use super::*;
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::template::TemplateKind;
use crate::testing::{FakeTemplateRepo, MockClock};

const REMOVED: ProjectId = ProjectId::from_raw(1);
const KEPT: ProjectId = ProjectId::from_raw(2);
const PROMPT: TemplateKind = TemplateKind::Prompt;

struct Setup {
    templates: Arc<Templates>,
    repo: Arc<FakeTemplateRepo>,
    bus: EventBus,
}

/// A live evictor over a fresh aggregate, already past its cold start — so what the tests warm
/// afterwards is only dropped by an eviction the loop decides on, never by the start-up clear.
async fn setup() -> Setup {
    let repo = Arc::new(FakeTemplateRepo::new());
    let templates = Arc::new(Templates::new(repo.clone()));
    let bus = EventBus::new(64);
    tokio::spawn(TemplateEvictor::new(&templates, &bus, Arc::new(MockClock::new())).run());
    yield_many().await;
    Setup {
        templates,
        repo,
        bus,
    }
}

async fn yield_many() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

/// Creates a prompt in the project's scope and lists that scope, leaving its rows cached.
fn warm(s: &Setup, project: ProjectId, name: &str) {
    s.templates
        .create(PROMPT, Some(project), name, None, "body")
        .expect("create the template");
    assert_eq!(
        s.templates
            .list(PROMPT, Some(project))
            .expect("warm the cache")
            .len(),
        1,
        "the scope starts warm with its one row"
    );
}

/// Deletes the row underneath the aggregate, as the store's foreign-key cascade does when the
/// project is removed — no write passes through `Templates`, so nothing invalidates its cache.
fn cascade_away(s: &Setup, project: ProjectId, name: &str) {
    assert!(
        TemplateRepo::delete(&*s.repo, PROMPT, Some(project), name).expect("delete the row"),
        "the row was there to cascade away"
    );
}

#[tokio::test]
async fn a_removed_projects_rows_are_not_served_from_the_cache_afterwards() {
    let s = setup().await;
    warm(&s, REMOVED, "review");
    cascade_away(&s, REMOVED, "review");

    s.bus.publish(DomainEvent::ProjectRemoved { id: REMOVED });
    yield_many().await;

    assert_eq!(
        s.templates
            .list(PROMPT, Some(REMOVED))
            .expect("list the removed project's scope"),
        Vec::new(),
        "a removed project's templates must not survive in the cache"
    );
}

#[tokio::test]
async fn removing_one_project_leaves_another_projects_cache_warm() {
    let s = setup().await;
    warm(&s, REMOVED, "review");
    warm(&s, KEPT, "ship");
    let scans = s.repo.list_calls();

    s.bus.publish(DomainEvent::ProjectRemoved { id: REMOVED });
    yield_many().await;

    assert_eq!(
        s.templates
            .list(PROMPT, Some(KEPT))
            .expect("list the kept project's scope")
            .len(),
        1,
        "the surviving project still reads its own row"
    );
    assert_eq!(
        s.repo.list_calls(),
        scans,
        "an unrelated project's cache entry is left warm, not dropped with the removed one"
    );
}

#[tokio::test]
async fn an_event_that_is_not_a_removal_evicts_nothing() {
    let s = setup().await;
    warm(&s, REMOVED, "review");
    let scans = s.repo.list_calls();

    s.bus.publish(DomainEvent::ProjectOpened { id: KEPT });
    yield_many().await;

    s.templates
        .list(PROMPT, Some(REMOVED))
        .expect("list the still-open project's scope");
    assert_eq!(
        s.repo.list_calls(),
        scans,
        "opening a project invalidates nothing"
    );
}
