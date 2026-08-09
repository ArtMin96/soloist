//! A stand-in for the GitHub command-line tool, and the repository folder that tells it what to
//! answer.
//!
//! Real invocations, real arguments, real standard input, real exit statuses — only the service
//! behind them is a stand-in. That is the whole point: what the adapter is judged on is the
//! invocation it makes and what it does with the answer, and neither can be observed by mocking
//! the layer that makes it.
//!
//! The stand-in has to be found the way the real tool is, so it is put on `PATH` — a change to
//! this process, made once, before any test starts anything.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Once;

use tempfile::TempDir;

/// Where the stand-in answers are kept inside a repository folder. Read by the stand-in from its
/// own working directory, which is the folder the adapter runs it in.
const ANSWERS: &str = ".gh-fake";

/// The stand-in itself: it records what it was asked, keeps anything it was given on standard
/// input, and answers with whatever the folder says — including the exit status, since telling
/// one failure from another is most of what the adapter does.
const STAND_IN: &str = r#"#!/bin/sh
answers="$PWD/.gh-fake"
key="$1${2:+-$2}"
mkdir -p "$answers"
printf '%s\n' "$*" >> "$answers/$key.asked"
if [ "$key" = "pr-create" ]; then cat > "$answers/pr-create.given"; fi
if [ -f "$answers/$key.wrote" ]; then cat "$answers/$key.wrote" >&2; fi
if [ -f "$answers/$key.answer" ]; then cat "$answers/$key.answer"; fi
if [ -f "$answers/$key.status" ]; then exit "$(cat "$answers/$key.status")"; fi
exit 0
"#;

static ON_PATH: Once = Once::new();

/// Puts the stand-in where the tool would be found, once for the whole test binary.
///
/// Every test in this binary wants the same stand-in, so the change is made once and every caller
/// waits for it — which is what keeps a process-wide change out of the way of tests running beside
/// each other.
fn on_path() {
    ON_PATH.call_once(|| {
        let bin = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gh-stand-in");
        fs::create_dir_all(&bin).expect("stand-in folder");
        let tool = bin.join("gh");
        fs::write(&tool, STAND_IN).expect("write the stand-in");
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).expect("make it runnable");
        let existing = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{existing}", bin.display()));
    });
}

/// A repository folder the stand-in answers for.
pub struct Repository {
    dir: TempDir,
}

impl Repository {
    /// A folder with the stand-in on `PATH` and no answers set, so every request succeeds with
    /// nothing to say until a test says otherwise.
    pub fn new() -> Self {
        on_path();
        let dir = TempDir::new().expect("temp dir");
        fs::create_dir_all(dir.path().join(ANSWERS)).expect("answers folder");
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// What the stand-in writes to standard output for `key` — the sub-command, hyphenated
    /// (`auth-status`, `repo-view`, `pr-list`, `pr-create`).
    pub fn answering(self, key: &str, answer: &str) -> Self {
        self.put(&format!("{key}.answer"), answer);
        self
    }

    /// What it writes about itself, which is what a refusal's account is carried from.
    pub fn writing(self, key: &str, written: &str) -> Self {
        self.put(&format!("{key}.wrote"), written);
        self
    }

    /// The status it exits with.
    pub fn failing(self, key: &str, status: i32) -> Self {
        self.put(&format!("{key}.status"), &status.to_string());
        self
    }

    /// What it was asked, one invocation per line.
    pub fn asked(&self, key: &str) -> String {
        fs::read_to_string(self.dir.path().join(ANSWERS).join(format!("{key}.asked")))
            .unwrap_or_default()
    }

    /// What it was given on standard input.
    pub fn given(&self) -> String {
        fs::read_to_string(self.dir.path().join(ANSWERS).join("pr-create.given"))
            .unwrap_or_default()
    }

    fn put(&self, name: &str, content: &str) {
        fs::write(self.dir.path().join(ANSWERS).join(name), content).expect("write an answer");
    }
}
