use super::*;

#[test]
fn at_builds_a_loopback_base_url() {
    let client = Client::at(24678, "token");
    assert_eq!(client.url("/health"), "http://127.0.0.1:24678/health");
    assert_eq!(
        client.url("/processes/3/restart"),
        "http://127.0.0.1:24678/processes/3/restart"
    );
}

#[test]
fn not_running_renders_the_acceptance_message() {
    // The acceptance criterion: a clear "Soloist is not running" when the app is down.
    assert_eq!(CliError::NotRunning.to_string(), "Soloist is not running");
}

#[test]
fn carried_messages_render_verbatim() {
    let resolve = CliError::Resolve("no process named \"web\"".to_string());
    assert_eq!(resolve.to_string(), "no process named \"web\"");
    let request = CliError::Request("the API returned HTTP 500".to_string());
    assert_eq!(request.to_string(), "the API returned HTTP 500");
}
