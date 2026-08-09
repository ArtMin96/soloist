//! The query's answer read into conversations, and the two things decided before it is asked:
//! which service to ask, and how much to ask for.

use super::{args, host_of, threads};

/// A captured answer's shape, scrubbed — two conversations on one file, one settled.
const ANSWERED: &str = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"nodes":[
{"id":"PRRT_1","isResolved":false,"isOutdated":false,"path":"pkg/merge.go","line":421,
 "comments":{"nodes":[{"author":{"login":"octocat"},"body":"quote the path","url":"https://forge.example/pull/12#discussion_r1"},
                      {"author":{"login":"hubot"},"body":"agreed","url":"https://forge.example/pull/12#discussion_r2"}]}},
{"id":"PRRT_2","isResolved":true,"isOutdated":true,"path":"git/client.go","line":null,
 "comments":{"nodes":[{"author":null,"body":"add the delimiter","url":"https://forge.example/pull/12#discussion_r3"}]}}
]}}}}}"#;

#[test]
fn a_conversation_carries_where_it_hangs_and_everything_said_in_it() {
    let read = threads(ANSWERED.as_bytes()).expect("the answer is readable");

    assert_eq!(read[0].id, "PRRT_1");
    assert_eq!(read[0].path.as_deref(), Some("pkg/merge.go"));
    assert_eq!(read[0].line, Some(421));
    assert_eq!(read[0].comments.len(), 2);
    assert_eq!(read[0].comments[1].author, "hubot");
    assert_eq!(
        read[0].url.as_deref(),
        Some("https://forge.example/pull/12#discussion_r1"),
        "a thread is read where its first comment is",
    );
}

#[test]
fn a_settled_conversation_says_it_is_settled_rather_than_being_dropped() {
    let read = threads(ANSWERED.as_bytes()).expect("the answer is readable");

    assert!(
        read[1].resolved,
        "the argument is often what explains the code"
    );
    assert!(read[1].outdated);
    assert_eq!(
        read[1].line, None,
        "a conversation whose lines have moved keeps its file and loses its line",
    );
}

#[test]
fn an_answer_that_could_not_be_read_is_a_failure_rather_than_no_conversations() {
    assert!(
        threads(br#"{"errors":[{"message":"Could not resolve to a Repository"}]}"#).is_err(),
        "reporting no conversations for a question that was never answered would say the argument \
         had been settled",
    );
}

#[test]
fn which_service_is_asked_comes_from_the_address_the_service_itself_reported() {
    assert_eq!(
        host_of("https://ghe.example.com/org/repo/pull/1"),
        Some("ghe.example.com"),
        "an enterprise repository has to answer for itself; the escape hatch would otherwise \
         resolve the account's own default service",
    );
    assert_eq!(
        host_of("https://github.com/cli/cli/pull/13955"),
        Some("github.com"),
    );
    assert_eq!(host_of("not an address"), None);
}

#[test]
fn the_ceiling_reaches_the_request_rather_than_only_the_answer() {
    let asked = args("github.com", 12, 7, 3);

    assert!(
        asked.contains(&"threads=7".to_string()) && asked.contains(&"comments=3".to_string()),
        "asking for everything and keeping a slice is a bounded slice of an unbounded request: \
         {asked:?}",
    );
    assert!(
        asked.contains(&"--hostname".to_string()) && asked.contains(&"github.com".to_string()),
        "the service is named outright: {asked:?}",
    );
}
