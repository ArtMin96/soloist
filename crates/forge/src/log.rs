//! Fetching what a failing check printed.
//!
//! Only checks the service's own runner produced have a log this can reach, and the only handle on
//! one is the address the check reported. That address is a page a person opens, not an interface —
//! so it is read for the job it names, and an address that does not name a job simply has no log.
//! Answering "there is none" is what keeps a check reported by somebody else's system from looking
//! like a failure.

/// Where a job's identifier sits in the address of the page that shows it.
const JOB_SEGMENT: &str = "/job/";

/// The arguments asking for what a job printed where it failed.
pub(crate) fn args(job: &str) -> Vec<String> {
    vec![
        "run".to_string(),
        "view".to_string(),
        "--job".to_string(),
        job.to_string(),
        "--log-failed".to_string(),
    ]
}

/// The job an address names, or `None` when it names none.
pub(crate) fn job_of(url: &str) -> Option<&str> {
    let after = url.split_once(JOB_SEGMENT)?.1;
    let job = after.split('/').next()?;
    (!job.is_empty() && job.bytes().all(|byte| byte.is_ascii_digit())).then_some(job)
}

/// The last `limit` bytes of `output` as text, cut at a line boundary so nothing is handed over as
/// half a line.
///
/// The end rather than the beginning: a failure is at the end of a log, and the beginning of one is
/// the machine describing itself.
pub(crate) fn tail(output: &[u8], limit: usize) -> String {
    let text = String::from_utf8_lossy(output);
    if text.len() <= limit {
        return text.into_owned();
    }
    // Walked forward to a character boundary rather than cut at the byte: a log is somebody else's
    // bytes, and a slice through the middle of a character is a panic.
    let mut from = text.len() - limit;
    while from < text.len() && !text.is_char_boundary(from) {
        from += 1;
    }
    let kept = &text[from..];
    match kept.find('\n') {
        Some(newline) => kept[newline + 1..].to_string(),
        None => kept.to_string(),
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
