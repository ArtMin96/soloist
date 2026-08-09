//! What the adapter reports on a host where the GitHub command-line tool was never installed.
//!
//! Emptying `PATH` is what makes the tool unfindable, and that change is process-wide — so this
//! test lives in a binary of its own, where it cannot reach any other test.

use soloist_core::{ForgeError, ForgeReadiness, GitForge, NewPullRequest, Stop};
use soloist_forge::GhForge;

#[test]
fn a_host_without_the_tool_offers_nothing_rather_than_failing_at_every_request() {
    std::env::set_var("PATH", "");
    let dir = tempfile::tempdir().expect("temp dir");
    let forge = GhForge::new();

    assert_eq!(
        forge.readiness(dir.path()),
        ForgeReadiness::Missing,
        "a surface reads this as there being nothing to offer, and says how to get it",
    );
    assert!(matches!(
        forge.create(
            dir.path(),
            "feature",
            &NewPullRequest {
                title: "Propose the thing".to_string(),
                body: String::new(),
                base: "main".to_string(),
                draft: false,
            },
            &Stop::default(),
        ),
        Err(ForgeError::Missing),
    ));
}
