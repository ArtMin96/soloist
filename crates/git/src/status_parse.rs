//! Turning `git status --porcelain=v2 -z --branch` into the core's working-tree vocabulary.
//!
//! This is the anti-corruption seam: the tool's records go in, domain values come out, and
//! nothing in between reaches the core. The format is the documented machine-readable one —
//! NUL-separated records, fixed field counts, fixed letters — so no output meant for a person,
//! and therefore no translation, is ever read.

use soloist_core::{BranchInfo, ChangeKind, FileChange, GitFileStatus, GitStatus, SyncState};

/// A header record, carrying what is checked out rather than a change.
const HEADER: &[u8] = b"# ";
/// A change to a tracked path that was neither renamed nor copied.
const ORDINARY: &[u8] = b"1 ";
/// A renamed or copied path. Its original path is the record that follows.
const RENAMED: &[u8] = b"2 ";
/// A path left unresolved by a merge.
const UNMERGED: &[u8] = b"u ";
/// A path version control does not track.
const UNTRACKED: &[u8] = b"? ";
/// A path version control was told to ignore. Never asked for here, so one arriving is not a
/// change to report.
const IGNORED: &[u8] = b"! ";

/// The header naming the checked-out branch.
const BRANCH_HEAD: &str = "# branch.head ";
/// The header naming what that branch tracks.
const BRANCH_UPSTREAM: &str = "# branch.upstream ";
/// The header carrying how far each side is ahead of the other, as `+<ahead> -<behind>`.
const BRANCH_AB: &str = "# branch.ab ";
/// What [`BRANCH_HEAD`] reports when nothing is checked out by name.
const DETACHED: &str = "(detached)";

/// Fields in an ordinary record: `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`.
const ORDINARY_FIELDS: usize = 9;
/// Fields in a rename or copy record: the ordinary ones plus a similarity score, before the
/// path. The original path is the next record, not a field.
const RENAMED_FIELDS: usize = 10;
/// Fields in an unmerged record: `u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`.
const UNMERGED_FIELDS: usize = 11;

/// Reads a status report, or `None` when it is not one this adapter recognises.
pub(crate) fn parse(output: &[u8]) -> Option<GitStatus> {
    let records: Vec<&[u8]> = output
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
        .collect();

    let mut name = None;
    let mut upstream = None;
    let mut counts = None;
    let mut changes = Vec::new();
    let mut at = 0;
    while at < records.len() {
        let record = records[at];
        at += 1;
        if record.starts_with(HEADER) {
            let header = std::str::from_utf8(record).ok()?;
            if let Some(head) = header.strip_prefix(BRANCH_HEAD) {
                name = (head != DETACHED).then(|| head.to_string());
            } else if let Some(tracked) = header.strip_prefix(BRANCH_UPSTREAM) {
                upstream = Some(tracked.to_string());
            } else if let Some(ab) = header.strip_prefix(BRANCH_AB) {
                counts = Some(ahead_behind(ab)?);
            }
        } else if record.starts_with(ORDINARY) {
            changes.push(tracked(record, ORDINARY_FIELDS, None)?);
        } else if record.starts_with(RENAMED) {
            // The original path is the record after this one, not a field within it.
            let original = records.get(at)?;
            at += 1;
            changes.push(tracked(record, RENAMED_FIELDS, Some(path_of(original)))?);
        } else if record.starts_with(UNMERGED) {
            changes.push(unmerged(record)?);
        } else if record.starts_with(UNTRACKED) {
            changes.push(untracked(record));
        } else if !record.starts_with(IGNORED) {
            return None;
        }
    }

    Some(GitStatus {
        branch: BranchInfo {
            name,
            upstream,
            // Counts arrive only when the upstream's position is known here, so their absence
            // is exactly the case that cannot be compared.
            sync: counts.map_or(SyncState::Unknown, |(ahead, behind)| {
                SyncState::from_counts(ahead, behind)
            }),
        },
        changes,
        // Whether a merge is under way is not in this report; the caller asks for it separately.
        merging: false,
    })
}

/// A change to a tracked path: the status pair says what happened on each side of the index.
fn tracked(record: &[u8], fields: usize, original_path: Option<String>) -> Option<FileChange> {
    let (pair, path) = split_record(record, fields)?;
    Some(FileChange {
        path,
        status: file_status(pair)?,
        original_path,
    })
}

/// A path a merge left unresolved. The status pair on such a record spells which sides changed;
/// every combination means the same thing to a surface — the file needs resolving.
fn unmerged(record: &[u8]) -> Option<FileChange> {
    let (_, path) = split_record(record, UNMERGED_FIELDS)?;
    Some(FileChange {
        path,
        status: GitFileStatus {
            staged: None,
            unstaged: Some(ChangeKind::Conflicted),
        },
        original_path: None,
    })
}

/// A path version control does not track. The record is the marker and the path, nothing else.
fn untracked(record: &[u8]) -> FileChange {
    FileChange {
        path: path_of(&record[UNTRACKED.len()..]),
        status: GitFileStatus {
            staged: None,
            unstaged: Some(ChangeKind::Untracked),
        },
        original_path: None,
    }
}

/// Splits a change record into its status pair and its path, given how many space-separated
/// fields the record kind has in total. The path is the last of them, and is the only one that
/// may itself contain spaces.
fn split_record(record: &[u8], fields: usize) -> Option<(&[u8], String)> {
    let mut parts = record.splitn(fields, |&byte| byte == b' ');
    parts.next()?;
    let pair = parts.next()?;
    for _ in 0..fields - 3 {
        parts.next()?;
    }
    Some((pair, path_of(parts.next()?)))
}

/// The staged and unstaged halves of a status pair.
fn file_status(pair: &[u8]) -> Option<GitFileStatus> {
    let [staged, unstaged] = pair else {
        return None;
    };
    Some(GitFileStatus {
        staged: change_kind(*staged)?,
        unstaged: change_kind(*unstaged)?,
    })
}

/// One half of a status pair: the change it names, or `None` for the unchanged side. The pair
/// fails to parse for a letter the format does not document, rather than being guessed at.
fn change_kind(letter: u8) -> Option<Option<ChangeKind>> {
    Some(match letter {
        b'.' => None,
        b'M' => Some(ChangeKind::Modified),
        b'T' => Some(ChangeKind::TypeChanged),
        b'A' => Some(ChangeKind::Added),
        b'D' => Some(ChangeKind::Deleted),
        b'R' => Some(ChangeKind::Renamed),
        b'C' => Some(ChangeKind::Copied),
        _ => return None,
    })
}

/// How many commits each side holds that the other does not, from a `+<ahead> -<behind>` field.
fn ahead_behind(field: &str) -> Option<(u32, u32)> {
    let (ahead, behind) = field.split_once(' ')?;
    Some((
        ahead.strip_prefix('+')?.parse().ok()?,
        behind.strip_prefix('-')?.parse().ok()?,
    ))
}

/// A path as version control reported it. Paths are raw bytes; one that is not valid UTF-8 is
/// carried lossily, since the read model it feeds crosses a boundary that could not represent
/// it faithfully either.
fn path_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
