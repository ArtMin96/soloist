//! What the adapter asks the tool for, and what it makes of the answer — driven through a
//! stand-in on `PATH`, so every invocation, argument, byte of standard input and exit status is
//! real.

mod fixture;

use fixture::Repository;
use soloist_core::{ForgeError, ForgeReadiness, GitForge, NewPullRequest, PullRequestState, Stop};
use soloist_forge::GhForge;

const BRANCH: &str = "feature";
const BASE: &str = "main";
const URL: &str = "https://github.example/owner/repo/pull/12";

/// What `gh auth status --json hosts` prints for an account that is signed in, scrubbed.
const SIGNED_IN: &str = r#"{"hosts":{"github.com":[{"state":"success","active":true,"host":"github.com","login":"octocat","tokenSource":"keyring","scopes":"gist, read:org, repo, workflow","gitProtocol":"https"}]}}"#;

/// A proposal a test hands over, varied by whichever field it is about.
fn proposal() -> NewPullRequest {
    NewPullRequest {
        title: "Propose the thing".to_string(),
        body: "## What changed\n\nA line with \"quotes\" and a $variable.\n".to_string(),
        base: BASE.to_string(),
        draft: false,
    }
}

#[test]
fn an_account_on_any_host_means_the_forge_can_be_reached() {
    let repository = Repository::new().answering("auth-status", SIGNED_IN);

    assert_eq!(
        GhForge::new().readiness(repository.path()),
        ForgeReadiness::Ready,
    );
}

#[test]
fn a_tool_signed_in_to_nothing_is_told_apart_from_one_that_is_not_installed() {
    let repository = Repository::new().answering("auth-status", r#"{"hosts":{}}"#);

    assert_eq!(
        GhForge::new().readiness(repository.path()),
        ForgeReadiness::LoggedOut,
        "one of these is fixed by installing something and the other by signing in, so a surface \
         has to be able to say which",
    );
}

#[test]
fn a_repositorys_default_branch_is_what_a_new_proposal_starts_from() {
    let repository =
        Repository::new().answering("repo-view", r#"{"defaultBranchRef":{"name":"trunk"}}"#);

    assert_eq!(
        GhForge::new()
            .default_base(repository.path())
            .expect("read"),
        Some("trunk".to_string()),
    );
}

#[test]
fn a_branch_nobody_has_proposed_yet_reports_none_rather_than_failing() {
    let repository = Repository::new().answering("pr-list", "[]");

    assert_eq!(
        GhForge::new()
            .pull_request(repository.path(), BRANCH)
            .expect("read"),
        None,
    );
}

#[test]
fn a_branch_that_already_has_one_reports_what_the_service_says_about_it() {
    let listed = format!(
        r#"[{{"baseRefName":"{BASE}","headRefName":"{BRANCH}","isDraft":false,"number":12,"state":"MERGED","title":"Propose the thing","url":"{URL}"}}]"#
    );
    let repository = Repository::new().answering("pr-list", &listed);

    let found = GhForge::new()
        .pull_request(repository.path(), BRANCH)
        .expect("read")
        .expect("one proposal");

    assert_eq!(found.number, 12);
    assert_eq!(found.state, PullRequestState::Merged);
    assert_eq!(found.url, URL);
}

#[test]
fn a_proposal_answers_with_where_what_it_made_can_be_found() {
    let repository = Repository::new().answering("pr-create", &format!("{URL}\n"));

    let made = GhForge::new()
        .create(repository.path(), BRANCH, &proposal(), &Stop::default())
        .expect("proposed");

    assert_eq!(made, URL);
}

#[test]
fn the_description_reaches_the_tool_over_standard_input_rather_than_on_its_command_line() {
    let repository = Repository::new().answering("pr-create", URL);

    GhForge::new()
        .create(repository.path(), BRANCH, &proposal(), &Stop::default())
        .expect("proposed");

    assert_eq!(
        repository.given(),
        proposal().body,
        "a person's prose has quotes, dollars and newlines in it, and none of them survive a \
         command line intact",
    );
    let asked = repository.asked("pr-create");
    assert!(
        !asked.contains("What changed"),
        "and it must not have been on the command line as well: {asked}",
    );
}

#[test]
fn a_proposal_asked_for_as_a_draft_says_so_and_one_that_is_not_stays_silent() {
    let ordinary = Repository::new().answering("pr-create", URL);
    GhForge::new()
        .create(ordinary.path(), BRANCH, &proposal(), &Stop::default())
        .expect("proposed");

    let drafted = Repository::new().answering("pr-create", URL);
    GhForge::new()
        .create(
            drafted.path(),
            BRANCH,
            &NewPullRequest {
                draft: true,
                ..proposal()
            },
            &Stop::default(),
        )
        .expect("proposed");

    assert!(!ordinary.asked("pr-create").contains("--draft"));
    assert!(
        drafted.asked("pr-create").contains("--draft"),
        "whether review is being asked for is decided entirely here, so it is the only place it \
         can be observed",
    );
}

#[test]
fn a_service_that_refuses_carries_its_own_account_of_why_back() {
    let repository = Repository::new().failing("pr-create", 1).writing(
        "pr-create",
        "a pull request for branch \"feature\" already exists",
    );

    let refusal = GhForge::new()
        .create(repository.path(), BRANCH, &proposal(), &Stop::default())
        .unwrap_err();

    assert!(
        matches!(&refusal, ForgeError::Refused { output } if output.contains("already exists")),
        "it is the service talking to the user, and nothing here could say it better: {refusal:?}",
    );
}

#[test]
fn a_request_that_needs_an_account_is_told_apart_from_every_other_failure() {
    let repository = Repository::new()
        .failing("pr-create", 4)
        .writing("pr-create", "authentication required");

    assert!(
        matches!(
            GhForge::new().create(repository.path(), BRANCH, &proposal(), &Stop::default()),
            Err(ForgeError::LoggedOut),
        ),
        "signing in is the one thing that fixes it, which no other failure's answer is",
    );
}

#[test]
fn a_failure_that_said_nothing_about_itself_keeps_the_status_it_had() {
    let repository = Repository::new().failing("pr-list", 1);

    assert!(matches!(
        GhForge::new().pull_request(repository.path(), BRANCH),
        Err(ForgeError::Op { status: Some(1) }),
    ));
}

#[test]
fn a_create_that_printed_no_address_is_a_failure_rather_than_an_empty_answer() {
    let repository = Repository::new();

    assert!(matches!(
        GhForge::new().create(repository.path(), BRANCH, &proposal(), &Stop::default()),
        Err(ForgeError::Op { status: None }),
    ));
}

#[test]
fn a_proposal_asked_to_stop_before_it_started_starts_nothing_at_all() {
    let repository = Repository::new().answering("pr-create", URL);
    let stop = Stop::default();
    stop.stop();

    let outcome = GhForge::new().create(repository.path(), BRANCH, &proposal(), &stop);

    assert!(matches!(outcome, Err(ForgeError::Stopped)), "{outcome:?}");
    assert_eq!(
        repository.asked("pr-create"),
        "",
        "being stopped before anything started costs no process at all",
    );
}
