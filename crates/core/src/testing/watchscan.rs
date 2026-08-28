//! A [`WatchScanner`] fake for the watch-set's tests: it answers from paths seeded per root
//! rather than touching the filesystem, so a test controls exactly what a scan finds and can
//! assert exactly what request produced it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::filewatch::{Scan, ScanRequest, ScannedPath, WatchScanner};
use crate::sync::lock;

/// An in-memory [`WatchScanner`] answering from paths seeded per root via [`Self::reporting`].
/// A root nothing was seeded for scans empty — an unseeded scan is a fact worth seeing in a
/// test's assertions, not a silent pass-through.
#[derive(Default)]
pub struct FakeWatchScanner {
    answers: Mutex<HashMap<PathBuf, Answer>>,
    requests: Mutex<Vec<ScanRequest>>,
    panicking: Mutex<Vec<PathBuf>>,
}

#[derive(Clone, Default)]
struct Answer {
    paths: Vec<ScannedPath>,
    truncated: bool,
}

impl FakeWatchScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds what scanning `root` reports: each `(path, directory)` pair becomes one
    /// [`ScannedPath`] in the answer.
    pub fn reporting(&self, root: impl Into<PathBuf>, paths: Vec<(&str, bool)>) {
        lock(&self.answers).entry(root.into()).or_default().paths = paths
            .into_iter()
            .map(|(path, directory)| ScannedPath {
                path: PathBuf::from(path),
                directory,
            })
            .collect();
    }

    /// Makes scanning `root` report [`Scan::truncated`], as a walk stopped at its ceiling does.
    pub fn truncating(&self, root: impl Into<PathBuf>) {
        lock(&self.answers)
            .entry(root.into())
            .or_default()
            .truncated = true;
    }

    /// Makes the next scan of `root` panic instead of answering, once — for exercising a
    /// supervised loop's restart deterministically. Consumed on the first matching scan; later
    /// scans of the same root answer normally.
    pub fn panicking_once(&self, root: impl Into<PathBuf>) {
        lock(&self.panicking).push(root.into());
    }

    /// Every request this scanner has answered, in the order it answered them — lets a test
    /// assert what ceiling and `honour_repository_ignores` it was given.
    pub fn requests(&self) -> Vec<ScanRequest> {
        lock(&self.requests).clone()
    }
}

impl WatchScanner for FakeWatchScanner {
    fn scan(&self, request: ScanRequest) -> Scan {
        {
            let mut panicking = lock(&self.panicking);
            if let Some(at) = panicking.iter().position(|root| *root == request.root) {
                panicking.remove(at);
                drop(panicking);
                // The fake panics by design to drive a supervised loop's restart.
                #[allow(clippy::panic)]
                {
                    panic!(
                        "FakeWatchScanner: forced panic scanning {}",
                        request.root.display()
                    );
                }
            }
        }
        lock(&self.requests).push(request.clone());
        let answer = lock(&self.answers)
            .get(&request.root)
            .cloned()
            .unwrap_or_default();
        Scan {
            paths: answer.paths,
            truncated: answer.truncated,
        }
    }
}
