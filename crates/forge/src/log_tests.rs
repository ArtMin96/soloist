//! Which addresses name something with a log, and how much of one is carried.

use super::{job_of, tail};

#[test]
fn a_checks_own_address_is_where_the_job_behind_it_is_named() {
    assert_eq!(
        job_of("https://github.com/cli/cli/actions/runs/30452791461/job/90578714966"),
        Some("90578714966"),
    );
}

#[test]
fn a_check_from_somewhere_other_than_the_services_own_runner_has_no_log_here() {
    for elsewhere in [
        "https://vercel.example/git/authorize?team=x&job=%7B%22id%22%3A%22Q%22%7D",
        "https://socket.dev/dashboard/org/vercel/sbom/00ecafe7",
        "https://github.com/cli/cli/actions/runs/30452791461",
        "https://github.com/cli/cli/actions/runs/1/job/not-a-number",
    ] {
        assert_eq!(
            job_of(elsewhere),
            None,
            "answering 'there is none' is what keeps somebody else's system from looking like a \
             failure: {elsewhere}",
        );
    }
}

#[test]
fn what_is_carried_is_the_end_of_the_log_because_that_is_where_the_failure_is() {
    let log = "setting up\nrunning\nerror: the thing is wrong\n";

    let kept = tail(log.as_bytes(), 30);

    assert!(kept.len() <= 30);
    assert!(
        kept.contains("error: the thing is wrong"),
        "the beginning of a log is the machine describing itself: {kept:?}",
    );
}

#[test]
fn a_log_cut_short_still_starts_at_a_line() {
    let log = "a line that is quite long indeed\nthe last line\n";

    let kept = tail(log.as_bytes(), 25);

    assert_eq!(
        kept, "the last line\n",
        "half a line reads as a different message from the one that was printed",
    );
}

#[test]
fn a_log_that_fits_is_carried_whole() {
    let log = "short\n";

    assert_eq!(tail(log.as_bytes(), 1024), log);
}

#[test]
fn a_log_that_is_not_text_is_still_answered_rather_than_bringing_the_read_down() {
    // A cut through the middle of a character is a panic, and a log is somebody else's bytes.
    let log = "ααααααααα".as_bytes();

    let kept = tail(log, 5);

    assert!(kept.len() <= 5);
}
