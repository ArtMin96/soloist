//! Behavioural tests for the façade's pull-request door — the one every adapter comes through.
//! They assemble a real [`Facade`] over fakes, so what is asserted is what a Tauri command, or a
//! tool, would get back.

use std::sync::Arc;

use tempfile::TempDir;

use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::git::{ForgeReadiness, NewPullRequest, PullRequestError};
use crate::ids::ProjectId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::template::TemplateKind;
use crate::testing::{
    commit_entry, created_url, described_entry, git_status, pull_request_template, tracking_status,
    FakeGitForge, FakeGitRepository, FakeProjectRepo, FakeSettingsRepo, FakeSpawner,
    FakeTemplateRepo, FakeTrustRepo,
};

const BRANCH: &str = "feature";
const BASE: &str = "main";
const MY_TEMPLATE: &str = "my shape";

/// A façade over both git fakes with one project open, returning it and the project's id. The
/// project is trusted or not as `trusted` says, which is the only gate a proposal passes.
fn facade_with_project(
    repository: FakeGitRepository,
    forge: FakeGitForge,
    trusted: bool,
) -> (Arc<Facade>, ProjectId, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let mut trust = FakeTrustRepo::new();
    if trusted {
        trust = trust.trusting_project(project);
    }
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(TokioClock),
        Arc::new(trust),
        projects,
    )
    .git_repository(Arc::new(repository))
    .git_forge(Arc::new(forge))
    .template_repo(Arc::new(FakeTemplateRepo::new()))
    .project_settings_repo(Arc::new(FakeSettingsRepo::new()))
    .build();
    (Arc::new(Facade::new(ports)), project, dir)
}

/// Puts a description template of the user's own in `project`'s library and selects it as the
/// default, which is the whole of how a personal shape comes to be offered.
fn select_own_template(facade: &Facade, project: ProjectId, body: &str) {
    let view = facade
        .template_create(TemplateKind::Pr, Some(project), MY_TEMPLATE, None, body)
        .expect("create");
    facade
        .set_default_template(TemplateKind::Pr, project, Some(view.id))
        .expect("select");
}

/// A proposal a test hands over, varied by whichever field it is about.
fn proposal() -> NewPullRequest {
    NewPullRequest {
        title: "Propose the thing".to_string(),
        body: "What it changes.".to_string(),
        base: BASE.to_string(),
        draft: false,
    }
}

#[test]
fn a_project_that_is_not_open_is_named_as_such_rather_than_reached() {
    let forge = FakeGitForge::ready();
    let (facade, _project, _dir) = facade_with_project(
        FakeGitRepository::reporting(git_status(BRANCH)),
        forge.clone(),
        true,
    );

    let refusal = facade
        .git_pull_request_surface(ProjectId::from_raw(4_242))
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::UnknownProject),
        "{refusal:?}",
    );
    assert_eq!(forge.asks(), 0);
}

#[test]
fn the_users_own_template_is_what_the_surface_offers_where_the_repository_carries_none() {
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(git_status(BRANCH)),
        FakeGitForge::ready(),
        true,
    );
    select_own_template(&facade, project, "## My own shape\n");

    let surface = facade.git_pull_request_surface(project).expect("surface");

    assert_eq!(
        surface.templates,
        vec![pull_request_template(MY_TEMPLATE, "## My own shape\n")],
        "a template kept in Soloist travels to every repository that expects nothing of its own",
    );
}

#[test]
fn a_repositorys_own_template_wins_over_the_users_even_when_both_exist() {
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(git_status(BRANCH)),
        FakeGitForge::ready().carrying(vec![pull_request_template("house", "## The house shape")]),
        true,
    );
    select_own_template(&facade, project, "## My own shape\n");

    let surface = facade.git_pull_request_surface(project).expect("surface");

    assert_eq!(
        surface
            .templates
            .iter()
            .map(|template| template.name.as_str())
            .collect::<Vec<_>>(),
        vec!["house"],
    );
}

#[test]
fn a_surface_over_a_forge_that_is_not_installed_says_so_instead_of_offering_a_form() {
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(git_status(BRANCH)),
        FakeGitForge::at(ForgeReadiness::Missing),
        true,
    );
    select_own_template(&facade, project, "## My own shape\n");

    let surface = facade.git_pull_request_surface(project).expect("surface");

    assert_eq!(surface.readiness, ForgeReadiness::Missing);
    assert_eq!(
        surface.templates,
        Vec::new(),
        "there is nothing to fill in a shape for, so none is offered",
    );
}

#[test]
fn a_proposal_comes_back_with_where_it_can_be_found() {
    let forge = FakeGitForge::ready();
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature")),
        forge.clone(),
        true,
    );

    let created = facade
        .git_create_pull_request(project, &proposal())
        .expect("proposed");

    assert_eq!(created, created_url());
    assert_eq!(forge.created(), vec![proposal()]);
}

#[test]
fn one_read_carries_everything_a_proposal_needs_so_it_can_be_offered_as_a_button() {
    let forge = FakeGitForge::ready().merging_into(BASE);
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature")).proposing(vec![
            described_entry("0", "Add the thing", "Because the other thing needed it."),
        ]),
        forge.clone(),
        true,
    );

    let surface = facade.git_pull_request_surface(project).expect("surface");
    let suggestion = surface
        .suggestion
        .clone()
        .expect("a branch carrying a commit has something to propose");
    let created = facade
        .git_create_pull_request(
            project,
            &NewPullRequest {
                title: suggestion.title,
                body: suggestion.body,
                base: surface.base.expect("a base to merge into"),
                draft: false,
            },
        )
        .expect("proposed");

    assert_eq!(created, created_url());
    assert_eq!(
        forge.created(),
        vec![NewPullRequest {
            title: "Add the thing".to_string(),
            body: "Because the other thing needed it.".to_string(),
            base: BASE.to_string(),
            draft: false,
        }],
        "nothing was typed, and what the branch already said about itself is what was proposed",
    );
}

#[test]
fn a_branch_holding_nothing_its_base_does_not_is_offered_no_suggestion_rather_than_an_invented_one()
{
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature"))
            .proposing(Vec::new()),
        FakeGitForge::ready().merging_into(BASE),
        true,
    );

    let surface = facade.git_pull_request_surface(project).expect("surface");

    assert_eq!(surface.suggestion, None);
    assert_eq!(
        surface.base.as_deref(),
        Some(BASE),
        "the rest of the surface is still true, so it is still answered",
    );
}

#[test]
fn a_suggestion_whose_title_came_out_blank_is_refused_like_any_other_blank_title() {
    let forge = FakeGitForge::ready().merging_into(BASE);
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature"))
            .proposing(vec![commit_entry("0", "   ", "Somebody")]),
        forge.clone(),
        true,
    );

    let surface = facade.git_pull_request_surface(project).expect("surface");
    let suggestion = surface.suggestion.expect("a commit was proposed");

    let refusal = facade
        .git_create_pull_request(
            project,
            &NewPullRequest {
                title: suggestion.title,
                body: suggestion.body,
                base: BASE.to_string(),
                draft: false,
            },
        )
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::EmptyTitle),
        "a computed title is input like any other, and a blank one is still refused: {refusal:?}",
    );
    assert_eq!(forge.created(), Vec::new());
}

#[test]
fn a_proposal_in_a_project_nobody_trusted_is_refused_at_this_door_too() {
    let forge = FakeGitForge::ready();
    let (facade, project, _dir) = facade_with_project(
        FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature")),
        forge.clone(),
        false,
    );

    let refusal = facade
        .git_create_pull_request(project, &proposal())
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::Untrusted),
        "{refusal:?}"
    );
    assert_eq!(forge.created(), Vec::new());
}
