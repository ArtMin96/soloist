//! Turning `git ls-files -z` output into the core's file vocabulary.
//!
//! Like the status seam, this reads only the machine-readable form — NUL-separated paths, one
//! per record — so nothing meant for a person, and therefore no translation, is ever read.

use soloist_core::ProjectFile;

/// Reads one listing's paths, marking each as `ignored`. Records arrive NUL-separated, so a
/// path containing any other byte — including a newline — survives intact.
pub(crate) fn parse(output: &[u8], ignored: bool) -> impl Iterator<Item = ProjectFile> + '_ {
    output
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
        .map(move |record| ProjectFile {
            // Paths are raw bytes; one that is not valid UTF-8 is carried lossily, since the
            // read model it feeds crosses a boundary that could not represent it faithfully
            // either.
            path: String::from_utf8_lossy(record).into_owned(),
            ignored,
        })
}
