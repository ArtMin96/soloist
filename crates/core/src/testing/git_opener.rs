//! A [`FileOpener`] fake for the git context's tests: it opens nothing and keeps every path it
//! was asked to open, which is the only trace an open leaves when no desktop is real.
//!
//! An empty record is what a gate holding looks like from outside — the observation the trust
//! tests are made of, since a refusal that still reached the desktop would have opened the file.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::git::{FileOpener, OpenError};
use crate::sync::lock;

/// An in-memory [`FileOpener`]: every open succeeds and is recorded, unless it was built to
/// refuse.
#[derive(Clone, Default)]
pub struct FakeFileOpener {
    opened: Arc<Mutex<Vec<String>>>,
    refusal: Option<OpenError>,
}

impl FakeFileOpener {
    /// A desktop that opens whatever it is handed.
    pub fn new() -> Self {
        Self::default()
    }

    /// A desktop that refuses every open with `refusal` — how a test states that the path led
    /// somewhere the core could not see from its name alone.
    pub fn refusing(refusal: OpenError) -> Self {
        Self {
            refusal: Some(refusal),
            ..Self::default()
        }
    }

    /// Every path the desktop was asked to open, in order.
    pub fn opened(&self) -> Vec<String> {
        lock(&self.opened).clone()
    }
}

impl FileOpener for FakeFileOpener {
    fn open(&self, _root: &Path, path: &str) -> Result<(), OpenError> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        lock(&self.opened).push(path.to_string());
        Ok(())
    }
}
