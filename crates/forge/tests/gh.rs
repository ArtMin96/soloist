//! What the adapter asks the tool for, and what it makes of the answer — driven through a
//! stand-in on `PATH`, so every invocation, argument, byte of standard input and exit status is
//! real.

mod fixture;

use fixture::Repository;
use soloist_core::{
    CheckRun, CheckState, ForgeError, ForgeReadiness, GitForge, MergeMethod, NewPullRequest,
    PullRequestState, ReviewLimits, Stop,
};
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
    let repository = Repository::new().answering(
        "repo-view",
        r#"{"defaultBranchRef":{"name":"trunk"},"mergeCommitAllowed":true,"squashMergeAllowed":false,"rebaseMergeAllowed":false,"viewerDefaultMergeMethod":"MERGE"}"#,
    );

    let read = GhForge::new().repository(repository.path()).expect("read");

    assert_eq!(read.default_base, Some("trunk".to_string()));
    assert_eq!(read.merge_methods, vec![MergeMethod::Merge]);
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

/// How much of somebody else's discussion one review test carries. Small, and the two halves
/// different, so a request handed one in place of the other is a failure rather than a coincidence.
const LIMITS: ReviewLimits = ReviewLimits {
    threads: 5,
    comments: 3,
};

/// What `gh pr list --json …,statusCheckRollup,reviews,comments` prints for a branch under review.
fn listed_under_review() -> String {
    format!(
        r#"[{{"baseRefName":"{BASE}","headRefName":"{BRANCH}","isDraft":false,"number":12,"state":"OPEN","title":"Propose the thing","url":"{URL}",
        "statusCheckRollup":[
          {{"__typename":"CheckRun","name":"build","status":"COMPLETED","conclusion":"FAILURE","workflowName":"Tests","detailsUrl":"https://github.example/owner/repo/actions/runs/9/job/77"}}
        ],
        "reviews":[{{"id":"PRR_1","author":{{"login":"octocat"}},"body":"needs work"}}],
        "comments":[]}}]"#
    )
}

/// What the query answers with for one settled conversation on a line of the diff.
const ANSWERED_THREADS: &str = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[
{"id":"PRRT_1","isResolved":false,"isOutdated":false,"path":"src/main.rs","line":42,
 "comments":{"nodes":[{"author":{"login":"hubot"},"body":"this leaks","url":"https://github.example/owner/repo/pull/12#discussion_r1"}]}}
]}}}}}"#;

#[test]
fn a_branch_under_review_reads_back_with_its_checks_and_every_conversation_on_it() {
    let repository = Repository::new()
        .answering("pr-list", &listed_under_review())
        .answering("api-graphql", ANSWERED_THREADS);

    let review = GhForge::new()
        .review(repository.path(), BRANCH, LIMITS)
        .expect("read")
        .expect("the branch has one open");

    assert_eq!(review.pull_request.number, 12);
    assert_eq!(review.checks[0].name, "build");
    assert_eq!(review.checks[0].state, CheckState::Failed);
    assert_eq!(
        review.threads[0].path.as_deref(),
        Some("src/main.rs"),
        "a comment on a line is what a reader came to read, so it comes first",
    );
    assert_eq!(review.threads[1].comments[0].body, "needs work");
}

#[test]
fn a_branch_with_nothing_open_reads_as_nothing_and_asks_no_further_questions() {
    let repository = Repository::new().answering("pr-list", "[]");

    assert!(GhForge::new()
        .review(repository.path(), BRANCH, LIMITS)
        .expect("read")
        .is_none());
    assert_eq!(
        repository.asked("api-graphql"),
        "",
        "there is no pull request to ask about, so nothing was asked about one",
    );
}

#[test]
fn the_conversations_are_asked_of_the_service_the_pull_requests_own_address_names() {
    let repository = Repository::new()
        .answering("pr-list", &listed_under_review())
        .answering("api-graphql", ANSWERED_THREADS);

    GhForge::new()
        .review(repository.path(), BRANCH, LIMITS)
        .expect("read");

    let asked = repository.asked("api-graphql");
    assert!(
        asked.contains("--hostname github.example"),
        "the escape hatch resolves its own host from the account rather than from the repository, \
         so an enterprise repository has to be named outright: {asked}",
    );
    assert!(
        asked.contains("threads=5") && asked.contains("comments=3"),
        "the core's ceiling reaches the request rather than only the answer: {asked}",
    );
}

#[test]
fn a_merge_names_the_pull_request_and_the_way_it_was_asked_for() {
    for (method, flag) in [
        (MergeMethod::Merge, "--merge"),
        (MergeMethod::Squash, "--squash"),
        (MergeMethod::Rebase, "--rebase"),
    ] {
        let repository = Repository::new();

        GhForge::new()
            .merge(repository.path(), 12, method, &Stop::default())
            .expect("merged");

        let asked = repository.asked("pr-merge");
        assert!(
            asked.contains("pr merge 12") && asked.contains(flag),
            "with nobody at a terminal the tool cannot ask which was meant, so it is always \
             named: {asked}",
        );
    }
}

#[test]
fn a_merge_the_service_refuses_carries_its_own_account_of_why_back() {
    let repository = Repository::new().failing("pr-merge", 1).writing(
        "pr-merge",
        "Pull request is not mergeable: the base branch policy prohibits the merge.",
    );

    let refusal = GhForge::new()
        .merge(repository.path(), 12, MergeMethod::Squash, &Stop::default())
        .unwrap_err();

    assert!(
        matches!(&refusal, ForgeError::Refused { output } if output.contains("not mergeable")),
        "the rules are the repository's, and nothing local could state them better: {refusal:?}",
    );
}

#[test]
fn a_failing_checks_output_is_fetched_for_the_job_its_own_address_names() {
    let repository = Repository::new().answering("run-view", "setting up\nerror: it broke\n");

    let log = GhForge::new()
        .check_log(repository.path(), &failing_check(), 1024)
        .expect("read")
        .expect("the address names a job");

    assert!(log.contains("error: it broke"));
    assert!(
        repository.asked("run-view").contains("--job 77"),
        "the job is the only handle a check's address offers on its output: {}",
        repository.asked("run-view"),
    );
}

#[test]
fn a_check_from_somewhere_else_has_no_output_here_and_costs_no_process() {
    let repository = Repository::new();
    let elsewhere = CheckRun {
        url: Some("https://vercel.example/git/authorize?team=x".to_string()),
        ..failing_check()
    };

    assert_eq!(
        GhForge::new()
            .check_log(repository.path(), &elsewhere, 1024)
            .expect("read"),
        None,
    );
    assert_eq!(
        repository.asked("run-view"),
        "",
        "a check reported by somebody else's system is not a failure, and asking about it would \
         make it look like one",
    );
}

/// The failing check a log test asks about, whose address names a job on the service's own runner.
fn failing_check() -> CheckRun {
    CheckRun {
        name: "build".to_string(),
        state: CheckState::Failed,
        workflow: Some("Tests".to_string()),
        url: Some("https://github.example/owner/repo/actions/runs/9/job/77".to_string()),
    }
}

#[test]
fn more_conversation_than_the_ceiling_permits_comes_back_cut_to_it() {
    // The query's own `first:` bounds what the service sends about lines of the diff, but what the
    // pull-request commands report about the change as a whole is bounded by the tool rather than
    // by us — so the ceiling is applied to the answer too.
    let reviews: Vec<String> = (0..LIMITS.threads + 3)
        .map(|n| format!(r#"{{"id":"PRR_{n}","author":{{"login":"octocat"}},"body":"said {n}"}}"#))
        .collect();
    let listed = format!(
        r#"[{{"baseRefName":"{BASE}","headRefName":"{BRANCH}","isDraft":false,"number":12,"state":"OPEN","title":"t","url":"{URL}","statusCheckRollup":[],"reviews":[{}],"comments":[]}}]"#,
        reviews.join(",")
    );
    let repository = Repository::new()
        .answering("pr-list", &listed)
        .answering("api-graphql", ANSWERED_THREADS);

    let review = GhForge::new()
        .review(repository.path(), BRANCH, LIMITS)
        .expect("read")
        .expect("the branch has one open");

    assert_eq!(review.threads.len(), LIMITS.threads);
}
