//! Writing the Soloist guide into a project's agent instructions file as a managed,
//! marker-delimited section — re-running replaces the section in place, so the file never
//! accumulates duplicates and the user's own content around it is preserved untouched.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::guide::agent_guide;

/// Opens the managed section. An HTML comment, so it is invisible in rendered Markdown.
const SECTION_BEGIN: &str = "<!-- soloist:integration-guide:begin -->";
/// Closes the managed section.
const SECTION_END: &str = "<!-- soloist:integration-guide:end -->";
/// The heading the managed section carries inside the markers.
const SECTION_HEADING: &str = "## Working inside Soloist";

/// Which agent-instructions file in the project root to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationFile {
    AgentsMd,
    ClaudeMd,
}

impl IntegrationFile {
    /// The fixed file name in the project root. Only these two names are ever written —
    /// the caller cannot steer the write anywhere else.
    pub fn file_name(self) -> &'static str {
        match self {
            IntegrationFile::AgentsMd => "AGENTS.md",
            IntegrationFile::ClaudeMd => "CLAUDE.md",
        }
    }
}

/// What a guide write did: the file it landed in, and whether that file was newly created.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationWrite {
    pub path: PathBuf,
    pub created: bool,
}

/// Why writing the guide failed — always the file itself (read, write, or replace).
#[derive(Debug, thiserror::Error)]
pub enum IntegrationWriteError {
    #[error("cannot write {path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// The complete managed section: markers around the heading and the guide.
fn managed_section() -> String {
    format!(
        "{SECTION_BEGIN}\n{SECTION_HEADING}\n\n{guide}\n{SECTION_END}",
        guide = agent_guide()
    )
}

/// Writes the guide into `root`'s chosen instructions file. A missing file is created with
/// just the section; a file already carrying both markers has the span between them
/// replaced in place; any other file gets the section appended after a blank line. The
/// write goes to a temporary sibling and is renamed over the target, so a crash mid-write
/// never leaves the user's file truncated.
pub fn write_integration_guide(
    root: &Path,
    file: IntegrationFile,
) -> Result<IntegrationWrite, IntegrationWriteError> {
    let path = root.join(file.file_name());
    let io_err = |source| IntegrationWriteError::Io {
        path: path.clone(),
        source,
    };

    let existing = match std::fs::read_to_string(&path) {
        Ok(contents) => Some(contents),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(io_err(err)),
    };
    let created = existing.is_none();
    let contents = match existing {
        None => format!("{}\n", managed_section()),
        Some(contents) => updated(&contents),
    };

    let tmp = path.with_file_name(format!(".{}.soloist-tmp", file.file_name()));
    std::fs::write(&tmp, contents).map_err(io_err)?;
    std::fs::rename(&tmp, &path).map_err(io_err)?;
    Ok(IntegrationWrite { path, created })
}

/// An existing file with the fresh section in it: the marked span replaced when both
/// markers are present in order, otherwise the section appended after a blank line.
fn updated(contents: &str) -> String {
    let begin = contents.find(SECTION_BEGIN);
    let end = contents.find(SECTION_END).map(|at| at + SECTION_END.len());
    match (begin, end) {
        (Some(begin), Some(end)) if begin < end => {
            let mut updated = String::with_capacity(contents.len());
            updated.push_str(&contents[..begin]);
            updated.push_str(&managed_section());
            updated.push_str(&contents[end..]);
            updated
        }
        _ => format!(
            "{}\n\n{}\n",
            contents.trim_end_matches('\n'),
            managed_section()
        ),
    }
}

#[cfg(test)]
#[path = "integration_file_tests.rs"]
mod tests;
