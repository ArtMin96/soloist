//! Finding the description skeletons a repository carries as its own convention.
//!
//! Read from the working tree rather than asked of the service, for two reasons: the convention is
//! the repository's, so it answers for a repository nobody has pushed yet; and a template that is
//! on this disk is the one the user is about to edit, where one the service holds might be a
//! commit behind.
//!
//! The places looked in, and the names accepted, are the ones the tool underneath would itself
//! apply — `.github/`, then the root, then `docs/`; a single `pull_request_template` with a
//! Markdown, text, or no extension; or a `PULL_REQUEST_TEMPLATE` directory of several — matched
//! without regard to case, because that is how the convention is written in practice. The first
//! place that carries anything is the answer: a repository does not offer two sets of templates.

use std::fs;
use std::path::Path;

use soloist_core::PullRequestTemplate;

/// The folders a repository states its convention in, in the order the answer is taken from.
const PLACES: [&str; 3] = [".github", "", "docs"];

/// The name a description skeleton is written under, matched without regard to case — as both a
/// file with one of [`EXTENSIONS`] and a directory holding several.
const TEMPLATE_NAME: &str = "pull_request_template";

/// The extensions a skeleton may carry. The empty one is a file with no extension at all, which
/// the convention has always allowed.
const EXTENSIONS: [&str; 3] = ["md", "txt", ""];

/// The most skeletons one repository is offered from. A directory past this is not offering a
/// choice any more, and the read is bounded rather than however many files somebody committed.
const TEMPLATE_LIMIT: usize = 20;

/// The most of one skeleton that is read. Past this it has stopped being a shape to fill in, and
/// reading a file without a ceiling is how one pathological repository becomes an out-of-memory.
const SIZE_LIMIT: u64 = 64 * 1024;

/// Every description skeleton the repository at `root` carries, in the order to offer them, or
/// empty when it carries none.
pub(crate) fn detect(root: &Path) -> Vec<PullRequestTemplate> {
    PLACES
        .iter()
        .map(|place| root.join(place))
        .find_map(|place| Some(found(&place)).filter(|found| !found.is_empty()))
        .unwrap_or_default()
}

/// What one folder carries: a directory of skeletons if it holds one, otherwise the single
/// skeleton if it holds that.
fn found(place: &Path) -> Vec<PullRequestTemplate> {
    let Ok(entries) = fs::read_dir(place) else {
        return Vec::new();
    };
    let mut single = None;
    let mut directory = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !named(&path) {
            continue;
        }
        if path.is_dir() {
            directory = Some(path);
        } else if single.is_none() {
            single = Some(path);
        }
    }
    match directory {
        // A repository that committed both is offering the choice, so the choice is what it gets.
        Some(directory) => several(&directory),
        None => single.as_deref().and_then(read).into_iter().collect(),
    }
}

/// The skeletons inside a directory of them, by name so the order is the repository's own rather
/// than the filesystem's.
fn several(directory: &Path) -> Vec<PullRequestTemplate> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && accepted(path))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| read(path))
        .take(TEMPLATE_LIMIT)
        .collect()
}

/// Whether a path is the one the convention names, whichever way it is capitalised — as a file
/// with an accepted extension, or as the directory of the same name.
fn named(path: &Path) -> bool {
    stem(path).is_some_and(|stem| stem == TEMPLATE_NAME) && (path.is_dir() || accepted(path))
}

/// Whether a file's extension is one a skeleton may carry.
fn accepted(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    EXTENSIONS.contains(&extension.as_str())
}

/// A path's name without its extension, lowercased — the form the convention is matched in.
fn stem(path: &Path) -> Option<String> {
    Some(path.file_stem()?.to_string_lossy().to_lowercase())
}

/// One skeleton, or `None` when it is not there, is past the ceiling, or holds bytes that are not
/// text — none of which is a shape anybody could fill in.
fn read(path: &Path) -> Option<PullRequestTemplate> {
    if fs::metadata(path).ok()?.len() > SIZE_LIMIT {
        return None;
    }
    let body = fs::read_to_string(path).ok()?;
    Some(PullRequestTemplate {
        name: name_of(path),
        body,
    })
}

/// What a skeleton is offered under: its own file name without the extension, which is the only
/// thing telling one of several apart.
fn name_of(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "templates_tests.rs"]
mod tests;
