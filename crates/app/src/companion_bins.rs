//! Exports the companion binaries shipped beside the app — the `soloist-mcp` helper and
//! the `soloist-cli` client — into `<data dir>/bin`, the one path that stays valid on
//! every install format. A `.deb` keeps its binaries at a stable `/usr/bin`, but an
//! AppImage extracts to a fresh `/tmp/.mount_*` directory each launch, so a generated
//! MCP snippet pointing at a sibling would break on the next run. The exported copy is
//! refreshed on startup (byte-compared first, written via a temp sibling + rename so a
//! client launching the helper mid-refresh never executes a half-written file) and is
//! what the Integrations snippets reference.

use std::fs;
use std::io;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// The MCP helper binary's file name.
pub const MCP_HELPER_BIN: &str = "soloist-mcp";

/// The command-line client binary's file name.
pub const CLI_BIN: &str = "soloist-cli";

/// The data-directory subdirectory holding the exported copies.
const DATA_BIN_DIR: &str = "bin";

/// The exported copy's path inside the data directory for a companion binary.
pub fn exported_path(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join(DATA_BIN_DIR).join(name)
}

/// Whether `path` is a regular file the owner can execute — a mere name collision (a
/// directory, a data file) must not be handed to an MCP client as a command.
pub fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Refreshes the exported copies from the executable binaries sitting beside `exe`.
/// A missing sibling is skipped (a dev layout that never built the CLI, or a partial
/// install) and a per-binary failure is logged and skipped — exporting is a convenience,
/// never a launch blocker; the snippet resolver falls back to the sibling path.
pub fn refresh(exe: Option<PathBuf>, data_dir: &Path) {
    let Some(dir) = exe.as_deref().and_then(Path::parent) else {
        return;
    };
    for name in [MCP_HELPER_BIN, CLI_BIN] {
        let sibling = dir.join(name);
        if !is_executable_file(&sibling) {
            continue;
        }
        if let Err(err) = export(&sibling, &exported_path(data_dir, name)) {
            eprintln!("soloist: could not export {name} to the data directory ({err})");
        }
    }
}

/// Copies `src` over `dest` when their bytes differ, returning whether a copy happened.
/// The copy lands in a temp sibling first and is renamed into place; the mode bits (the
/// execute permission) come along with `fs::copy`. A failed step removes the temp file.
fn export(src: &Path, dest: &Path) -> io::Result<bool> {
    if same_contents(src, dest)? {
        return Ok(false);
    }
    let dir = dest
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent"))?;
    let name = dest
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "destination has no name"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.tmp", name.to_string_lossy()));
    let written = fs::copy(src, &tmp).and_then(|_| fs::rename(&tmp, dest));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written.map(|_| true)
}

/// Whether both files exist with identical bytes, compared in bounded buffered chunks
/// (the binaries are tens of megabytes; nothing is read when the sizes already differ).
/// A missing or unreadable `dest` is simply "different".
fn same_contents(src: &Path, dest: &Path) -> io::Result<bool> {
    let (Ok(src_meta), Ok(dest_meta)) = (fs::metadata(src), fs::metadata(dest)) else {
        return Ok(false);
    };
    if src_meta.len() != dest_meta.len() {
        return Ok(false);
    }
    let mut a = io::BufReader::new(fs::File::open(src)?);
    let Ok(file) = fs::File::open(dest) else {
        return Ok(false);
    };
    let mut b = io::BufReader::new(file);
    loop {
        let (chunk_a, chunk_b) = (a.fill_buf()?, b.fill_buf()?);
        if chunk_a.is_empty() && chunk_b.is_empty() {
            return Ok(true);
        }
        let len = chunk_a.len().min(chunk_b.len());
        if len == 0 || chunk_a[..len] != chunk_b[..len] {
            return Ok(false);
        }
        a.consume(len);
        b.consume(len);
    }
}

#[cfg(test)]
#[path = "companion_bins_tests.rs"]
mod tests;
