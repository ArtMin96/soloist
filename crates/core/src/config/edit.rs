//! Comment-preserving, stability-first editing of a `solo.yml`.
//!
//! [`rewrite`] turns a *current* parsed config plus an *intended* one into new file text. It edits
//! the `processes:` block in place so the user's comments, ordering, and formatting survive, then
//! **re-parses the result and verifies it equals the intended config**; if the in-place edit did
//! not reproduce it exactly (an unusual layout, an inline mapping, a quoted key it cannot parse), a
//! faithful full render — preserving the file's leading comment block — is returned instead. The
//! write therefore can never corrupt the file or write something other than the intended config:
//! correctness is guaranteed by the verify-and-fall-back frame, independent of the in-place editor.
//!
//! In-place editing requires the canonical two-space entry indentation `serde_norway` itself
//! writes (the demo's `solo.yml` and every file Soloist generates use it); any other layout falls
//! back to the always-correct render.

use std::collections::{HashMap, HashSet};

use super::load::parse;
use super::model::{ProcessSpec, SoloYml};
use super::write::WriteError;

/// Produces the new `solo.yml` text for `intended`, preserving `original`'s comments and formatting
/// where possible and never writing anything but the intended config. `current` is the config
/// `original` parses to, used to diff which entries changed.
pub(super) fn rewrite(
    original: &str,
    current: &SoloYml,
    intended: &SoloYml,
) -> Result<String, WriteError> {
    if let Some(text) = try_in_place(original, current, intended)? {
        // Only trust the in-place edit if it round-trips to exactly the intended config.
        if parse(&text).ok().as_ref() == Some(intended) {
            return Ok(text);
        }
    }
    fallback(original, intended)
}

/// The owned line spans of a `solo.yml`'s `processes:` block, split into the lines before the first
/// entry (`head`, reproduced verbatim), the entries themselves, and the lines after the block
/// (`tail`, reproduced verbatim).
struct Block {
    head: Vec<String>,
    entries: Vec<Entry>,
    tail: Vec<String>,
}

/// One parsed `processes:` entry: its decoded name plus its verbatim lines (the key line followed
/// by its field/comment/blank lines, up to the next entry).
struct Entry {
    name: String,
    lines: Vec<String>,
}

fn try_in_place(
    original: &str,
    current: &SoloYml,
    intended: &SoloYml,
) -> Result<Option<String>, WriteError> {
    let Some(block) = parse_block(original) else {
        return Ok(None);
    };

    let changes = super::diff::diff(current, intended);
    let removed: HashSet<&str> = changes.removed.iter().map(String::as_str).collect();
    let updated: HashSet<&str> = changes.updated.iter().map(String::as_str).collect();
    let renamed: HashMap<&str, &str> = changes
        .renamed
        .iter()
        .map(|r| (r.from.as_str(), r.to.as_str()))
        .collect();

    let mut out = block.head;
    for entry in block.entries {
        if removed.contains(entry.name.as_str()) {
            continue;
        }
        if let Some(&to) = renamed.get(entry.name.as_str()) {
            let Some(spec) = intended.processes.get(to) else {
                return Ok(None);
            };
            let rendered = entry_lines(to, spec)?;
            out.push(rendered[0].clone());
            // A pure rename keeps the body (and its comments) verbatim; a rename that also edits the
            // spec re-renders the fields.
            if current.processes.get(&entry.name) == intended.processes.get(to) {
                out.extend(entry.lines.into_iter().skip(1));
            } else {
                out.extend(rendered.into_iter().skip(1));
            }
            continue;
        }
        if updated.contains(entry.name.as_str()) {
            let Some(spec) = intended.processes.get(&entry.name) else {
                return Ok(None);
            };
            let rendered = entry_lines(&entry.name, spec)?;
            // Keep the original key line (preserving any trailing comment); re-render the fields.
            out.push(entry.lines[0].clone());
            out.extend(rendered.into_iter().skip(1));
            continue;
        }
        out.extend(entry.lines);
    }
    for name in &changes.added {
        let Some(spec) = intended.processes.get(name) else {
            return Ok(None);
        };
        out.extend(entry_lines(name, spec)?);
    }
    out.extend(block.tail);

    let mut result = out.join("\n");
    if original.ends_with('\n') {
        result.push('\n');
    }
    Ok(Some(result))
}

/// Splits `original` into its `processes:` block, or `None` when the file has no plain block mapping
/// under `processes:` to edit (an absent or inline `processes:`, or no entries) — the caller then
/// falls back to a full render.
fn parse_block(original: &str) -> Option<Block> {
    let trimmed = original.strip_suffix('\n').unwrap_or(original);
    let lines: Vec<&str> = trimmed.split('\n').collect();
    let proc = lines.iter().position(|line| *line == "processes:")?;

    // The block runs until a sibling top-level key (indent 0, non-blank, non-comment) or EOF.
    let mut end = proc + 1;
    while end < lines.len() {
        let line = lines[end];
        if is_blank(line) || is_comment(line) || indent(line) > 0 {
            end += 1;
        } else {
            break;
        }
    }

    let body = &lines[proc + 1..end];
    let first = body.iter().position(|line| is_entry_key(line))?;

    let mut head: Vec<String> = lines[..=proc].iter().map(|s| s.to_string()).collect();
    head.extend(body[..first].iter().map(|s| s.to_string()));

    let mut entries = Vec::new();
    let mut i = first;
    while i < body.len() {
        let name = parse_key(body[i])?;
        let mut entry_lines = vec![body[i].to_string()];
        i += 1;
        while i < body.len() && !is_entry_key(body[i]) {
            entry_lines.push(body[i].to_string());
            i += 1;
        }
        entries.push(Entry {
            name,
            lines: entry_lines,
        });
    }

    let tail = lines[end..].iter().map(|s| s.to_string()).collect();
    Some(Block {
        head,
        entries,
        tail,
    })
}

/// Renders one process entry (key line + field lines) at the canonical two-space indentation, by
/// serializing a single-entry config and dropping its `processes:` header line.
fn entry_lines(name: &str, spec: &ProcessSpec) -> Result<Vec<String>, WriteError> {
    let mut one = SoloYml::default();
    one.processes.insert(name.to_string(), spec.clone());
    let text = serde_norway::to_string(&one).map_err(WriteError::Serialize)?;
    let body = text.strip_prefix("processes:\n").unwrap_or(&text);
    Ok(body.lines().map(String::from).collect())
}

/// A faithful full render that preserves the file's leading comment block (never injecting
/// Soloist's own header into a file the user wrote) and re-serializes the data. Always correct.
fn fallback(original: &str, intended: &SoloYml) -> Result<String, WriteError> {
    let leading = leading_comment_block(original);
    let body = serde_norway::to_string(intended).map_err(WriteError::Serialize)?;
    Ok(format!("{leading}{body}"))
}

/// The file's leading run of blank and comment lines, reproduced verbatim with a trailing newline
/// so the rendered body starts on its own line. Empty when the file starts with data.
fn leading_comment_block(original: &str) -> String {
    let mut out = String::new();
    for line in original.split('\n') {
        if is_blank(line) || is_comment(line) {
            out.push_str(line);
            out.push('\n');
        } else {
            break;
        }
    }
    out
}

/// Decodes a two-space-indented entry key (plain, single-, or double-quoted) to its scalar name, or
/// `None` for any layout this minimal parser does not handle (which sends the caller to fallback).
fn parse_key(line: &str) -> Option<String> {
    let s = line.strip_prefix("  ")?.trim_end();
    match s.as_bytes().first()? {
        b'\'' => quoted_key(s, '\''),
        b'"' => quoted_key(s, '"'),
        _ => Some(s[..s.find(':')?].trim_end().to_string()),
    }
}

fn quoted_key(s: &str, quote: char) -> Option<String> {
    let rest = &s[1..];
    let end = rest.find(quote)?;
    if !rest[end + 1..].trim_start().starts_with(':') {
        return None;
    }
    Some(rest[..end].to_string())
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// A line that begins an entry: the canonical two-space indent, and not a blank or comment (an
/// entry's own fields sit at four spaces, so two-space depth uniquely marks an entry key).
fn is_entry_key(line: &str) -> bool {
    indent(line) == 2 && !is_blank(line) && !is_comment(line)
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
