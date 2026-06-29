use super::*;
use crate::ids::{ProjectId, ScratchpadId, TodoId};

#[test]
fn a_scratchpad_link_round_trips() {
    let link = Link::scratchpad(ProjectId::from_raw(7), ScratchpadId::from_raw(42));
    let text = link.to_link();
    assert_eq!(text, "solo://proj/7/scratchpad/42");
    assert_eq!(Link::parse(&text), Ok(link));
}

#[test]
fn a_todo_link_round_trips() {
    let link = Link::todo(ProjectId::from_raw(3), TodoId::from_raw(9));
    let text = link.to_link();
    assert_eq!(text, "solo://proj/3/todo/9");
    assert_eq!(Link::parse(&text), Ok(link));
}

#[test]
fn malformed_links_are_rejected() {
    for bad in [
        "",
        "solo://",
        "https://proj/1/todo/2",      // wrong scheme
        "solo://proj/1/todo",         // missing id
        "solo://proj/1/note/2",       // unknown kind
        "solo://team/1/todo/2",       // wrong project segment
        "solo://proj/x/todo/2",       // non-numeric project
        "solo://proj/1/todo/x",       // non-numeric id
        "solo://proj/1/todo/2/extra", // trailing path
    ] {
        assert_eq!(
            Link::parse(bad),
            Err(LinkError),
            "{bad:?} should be rejected"
        );
    }
}

#[test]
fn is_link_routes_only_the_solo_scheme() {
    assert!(is_link("solo://proj/1/todo/2"));
    assert!(
        is_link("solo://anything"),
        "a scheme match routes even if it won't fully parse"
    );
    assert!(!is_link("my-scratchpad"));
    assert!(!is_link("solo:/proj/1"));
}
