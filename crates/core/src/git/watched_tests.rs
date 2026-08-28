//! Behavioural tests for [`Watches::establish`]: whether a refusal that has since cleared is
//! re-attempted on a later resync, and whether a watch already granted is left alone rather than
//! churned.
//!
//! Driven directly against `Watches` over a [`FakeFileWatcher`] rather than through
//! [`super::super::GitStatusWatchReactor`] — no clock, no debounce, no repository read is under
//! test here, only what `establish` asks the watcher for on a second call.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::ids::ProjectId;
use crate::testing::FakeFileWatcher;
use crate::watch::WatchError;

use super::{Watches, REFS_DIR, STATE_DIR};

/// How many pending changed paths the test's channel buffers. Never drained — these tests assert
/// on what was asked for, not on a change reaching a receiver — so any small bound does.
const CHANGE_BUFFER: usize = 16;

fn setup() -> (Arc<FakeFileWatcher>, Watches) {
    let watcher = Arc::new(FakeFileWatcher::new());
    let (changes_tx, _changes_rx) = mpsc::channel(CHANGE_BUFFER);
    let watches = Watches::new(watcher.clone(), changes_tx);
    (watcher, watches)
}

fn root() -> PathBuf {
    PathBuf::from("/project")
}

fn state_dir(root: &std::path::Path) -> PathBuf {
    root.join(STATE_DIR)
}

fn refs_dir(root: &std::path::Path) -> PathBuf {
    state_dir(root).join(REFS_DIR)
}

/// How many times `path` was asked for.
fn asked(requested: &[PathBuf], path: &std::path::Path) -> usize {
    requested
        .iter()
        .filter(|asked| asked.as_path() == path)
        .count()
}

#[tokio::test]
async fn a_cleared_total_refusal_is_established_on_the_next_resync() {
    let (watcher, mut watches) = setup();
    let project = ProjectId::next();
    let root = root();
    let state_dir = state_dir(&root);
    let refs_dir = refs_dir(&root);
    watcher.refuse(root.clone());
    watcher.refuse(state_dir.clone());
    watcher.refuse(refs_dir.clone());

    let refused = watches.establish(project, root.clone()).await;
    assert_eq!(
        refused.refusal,
        Some(WatchError::BudgetExhausted),
        "the OS turned every watch down",
    );

    watcher.allow(root.clone());
    watcher.allow(state_dir.clone());
    watcher.allow(refs_dir.clone());
    let cleared = watches.establish(project, root.clone()).await;
    assert!(
        cleared.refusal.is_none(),
        "a total refusal that has since cleared must not be replayed for ever: {:?}",
        cleared.refusal,
    );
    assert!(
        watcher.live().contains(&root),
        "the working tree is actually watched once the refusal clears: {:?}",
        watcher.live(),
    );
}

/// The case a per-project bucket cannot fix: only the working tree is refused, so the cheap
/// repository-state watches succeed on the first attempt and must never be re-attempted, while the
/// refused tree is retried on every resync until it is granted.
#[tokio::test]
async fn a_partial_refusal_re_attempts_only_the_refused_path() {
    let (watcher, mut watches) = setup();
    let project = ProjectId::next();
    let root = root();
    let state_dir = state_dir(&root);
    watcher.refuse(root.clone());

    watches.establish(project, root.clone()).await;
    watches.establish(project, root.clone()).await;
    watcher.allow(root.clone());
    let cleared = watches.establish(project, root.clone()).await;

    let requested = watcher.watched();
    assert_eq!(
        asked(&requested, &state_dir),
        1,
        "the healthy repository-state watch was never churned: {requested:?}",
    );
    assert_eq!(
        asked(&requested, &root),
        3,
        "the refused working-tree watch was retried on every resync: {requested:?}",
    );
    assert!(
        cleared.refusal.is_none(),
        "the refusal clears once the OS grants it: {:?}",
        cleared.refusal,
    );
}

/// The sub-case a two-bucket (metadata/tree) design would still miss: `.git` and `.git/refs` are
/// two different paths, and a refusal of one must not churn the other.
#[tokio::test]
async fn a_refused_refs_tree_is_re_attempted_without_disturbing_the_state_dir() {
    let (watcher, mut watches) = setup();
    let project = ProjectId::next();
    let root = root();
    let state_dir = state_dir(&root);
    let refs_dir = refs_dir(&root);
    watcher.refuse(refs_dir.clone());

    let first = watches.establish(project, root.clone()).await;
    let second = watches.establish(project, root.clone()).await;

    let requested = watcher.watched();
    assert_eq!(
        asked(&requested, &refs_dir),
        2,
        "the refused refs tree was retried: {requested:?}",
    );
    assert_eq!(
        asked(&requested, &state_dir),
        1,
        "the granted state directory was left alone: {requested:?}",
    );
    assert_eq!(
        asked(&requested, &root),
        1,
        "the granted tree was left alone: {requested:?}"
    );
    assert!(
        first.refusal.is_none() && second.refusal.is_none(),
        "a refs refusal is not reported: {:?}, {:?}",
        first.refusal,
        second.refusal,
    );
}

#[tokio::test]
async fn a_granted_watch_is_not_re_established_on_resync() {
    let (watcher, mut watches) = setup();
    let project = ProjectId::next();
    let root = root();
    let state_dir = state_dir(&root);
    let refs_dir = refs_dir(&root);

    watches.establish(project, root.clone()).await;
    watches.establish(project, root.clone()).await;

    let requested = watcher.watched();
    assert_eq!(asked(&requested, &root), 1, "the root: {requested:?}");
    assert_eq!(
        asked(&requested, &state_dir),
        1,
        "the state dir: {requested:?}"
    );
    assert_eq!(
        asked(&requested, &refs_dir),
        1,
        "the refs tree: {requested:?}"
    );
}

#[tokio::test]
async fn only_the_working_trees_refusal_is_reported() {
    let (watcher, mut watches) = setup();
    let project = ProjectId::next();
    let root = root();
    watcher.refuse(state_dir(&root));

    let outcome = watches.establish(project, root.clone()).await;

    assert!(
        outcome.refusal.is_none(),
        "a project with no repository state is not one whose watching failed: {:?}",
        outcome.refusal,
    );
    assert!(
        watcher.live().contains(&root),
        "the working tree is watched regardless: {:?}",
        watcher.live(),
    );
}

/// Also proves `release` drops the whole `held` record, not only the granted watches: if it kept
/// the refusal and dropped only the handles, `.git` and `.git/refs` (never refused) would still
/// read as held and be left alone, while only the root would be asked for again.
#[tokio::test]
async fn releasing_a_project_forgets_its_refusal() {
    let (watcher, mut watches) = setup();
    let project = ProjectId::next();
    let root = root();
    let state_dir = state_dir(&root);
    let refs_dir = refs_dir(&root);
    watcher.refuse(root.clone());

    watches.establish(project, root.clone()).await;
    watcher.allow(root.clone());
    watches.release(project);
    let outcome = watches.establish(project, root.clone()).await;

    assert!(
        outcome.refusal.is_none(),
        "release forgets the standing refusal along with the handle: {:?}",
        outcome.refusal,
    );
    let requested = watcher.watched();
    assert_eq!(
        asked(&requested, &state_dir),
        2,
        "release cleared the whole record, so the state dir was asked for again too: {requested:?}",
    );
    assert_eq!(
        asked(&requested, &refs_dir),
        2,
        "release cleared the whole record, so the refs tree was asked for again too: {requested:?}",
    );
    assert_eq!(
        asked(&requested, &root),
        2,
        "the root was asked for again once the refusal cleared: {requested:?}",
    );
}
