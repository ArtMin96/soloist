//! Integration check against the real `notify` file watcher: a file created, changed, or removed
//! under a watched root is reported on the change channel, dropping the handle stops the watch, and
//! a root the OS will not watch comes back as a refusal rather than as a handle that stays quiet.
//! The mock-clock matching/debounce behaviour is covered in the core; this is where which *kinds* of
//! filesystem event reach the core at all is pinned, since that is the one decision this adapter
//! makes. Uses real time (a short poll budget), like the other OS-adapter integration tests.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use soloist_core::{FileWatcher, WatchError};
use soloist_sys::NotifyFileWatcher;
use tokio::sync::mpsc;

/// How long to wait for an inotify event before giving up — generous so a loaded CI box does
/// not flake, while a working watcher returns far sooner.
const BUDGET: Duration = Duration::from_secs(5);

/// A short window in which a stopped watch must stay silent — long enough that a working
/// watch would have delivered (events arrive in tens of ms), short enough to keep the test
/// quick.
const QUIET: Duration = Duration::from_millis(400);

/// Blocks until a changed path arrives or `budget` elapses, returning it if seen.
fn change_within(rx: &mut mpsc::Receiver<PathBuf>, budget: Duration) -> Option<PathBuf> {
    let deadline = Instant::now() + budget;
    loop {
        if let Ok(path) = rx.try_recv() {
            return Some(path);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn reports_a_file_created_under_a_watched_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let (tx, mut rx) = mpsc::channel(64);

    // The watch is established synchronously before watch() returns.
    let _handle = NotifyFileWatcher::new()
        .watch(root.clone(), tx)
        .expect("watch the root");

    let target = root.join("created.txt");
    fs::write(&target, b"hello").expect("write watched file");

    let change =
        change_within(&mut rx, BUDGET).expect("a create under the watched root is reported");
    assert!(
        change.ends_with("created.txt"),
        "expected the created file, got {change:?}",
    );
}

#[test]
fn reports_a_change_in_a_nested_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let nested = root.join("src").join("app");
    fs::create_dir_all(&nested).expect("nested dirs");
    let (tx, mut rx) = mpsc::channel(64);

    let _handle = NotifyFileWatcher::new()
        .watch(root.clone(), tx)
        .expect("watch the root");

    let target = nested.join("main.rs");
    fs::write(&target, b"fn main() {}").expect("write nested file");

    let change = change_within(&mut rx, BUDGET).expect("a recursive watch reports a nested change");
    assert!(
        change.ends_with("main.rs"),
        "expected the nested file, got {change:?}",
    );
}

#[test]
fn reports_a_file_removed_from_a_watched_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let target = root.join("doomed.txt");
    // Created before the watch starts, so the removal below is the only event the watch can
    // report — a create for the same path cannot be mistaken for it.
    fs::write(&target, b"hello").expect("write before watching");
    let (tx, mut rx) = mpsc::channel(64);

    let _handle = NotifyFileWatcher::new()
        .watch(root.clone(), tx)
        .expect("watch the root");

    fs::remove_file(&target).expect("remove watched file");

    let change = change_within(&mut rx, BUDGET)
        .expect("a removal under the watched root is reported: a deleted file changes the tree");
    assert!(
        change.ends_with("doomed.txt"),
        "expected the removed file, got {change:?}",
    );
}

#[test]
fn reports_a_file_renamed_within_a_watched_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let before = root.join("before.txt");
    fs::write(&before, b"hello").expect("write before watching");
    let (tx, mut rx) = mpsc::channel(64);

    let _handle = NotifyFileWatcher::new()
        .watch(root.clone(), tx)
        .expect("watch the root");

    // The backend reports a rename as a modification of the name rather than as a removal plus a
    // creation, so a rename reaching the core is a distinct fact from either of those.
    fs::rename(&before, root.join("after.txt")).expect("rename watched file");

    let renamed =
        change_within(&mut rx, BUDGET).expect("a rename under the watched root is reported");
    assert!(
        renamed.ends_with("before.txt") || renamed.ends_with("after.txt"),
        "expected one side of the rename, got {renamed:?}",
    );
}

#[test]
fn dropping_the_handle_stops_the_watch() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let (tx, mut rx) = mpsc::channel(64);

    let handle = NotifyFileWatcher::new()
        .watch(root.clone(), tx)
        .expect("watch the root");
    drop(handle);
    // Give the backend a moment to tear the watch down before changing the tree.
    std::thread::sleep(Duration::from_millis(100));
    while rx.try_recv().is_ok() {}

    fs::write(root.join("after_drop.txt"), b"x").expect("write after drop");

    assert!(
        change_within(&mut rx, QUIET).is_none(),
        "no changes are reported once the watch handle is dropped",
    );
}

#[test]
fn a_root_that_cannot_be_watched_says_so_rather_than_going_quiet() {
    let missing = PathBuf::from("/nonexistent/soloist/watch/root");
    let (tx, _rx) = mpsc::channel(64);

    let refused = NotifyFileWatcher::new()
        .watch(missing, tx)
        .err()
        .expect("a path that does not exist cannot be watched");

    // A handle that reports nothing is what an untouched tree looks like too, so a watch the OS
    // turns down has to come back as a refusal — otherwise the reactor above cannot tell a working
    // watch from a dead one, and the subsystem dies in silence.
    assert_eq!(
        refused,
        WatchError::Unwatchable,
        "a path that does not exist is refused, not silently unwatched",
    );
}

#[test]
fn a_dir_watch_reports_direct_children_but_not_nested_ones() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let nested = root.join("src");
    fs::create_dir_all(&nested).expect("nested dir");
    let (tx, mut rx) = mpsc::channel(64);

    // Non-recursive: exactly the depth a project root's `solo.yml` needs, at the cost of
    // one watch descriptor however large the tree is.
    let _handle = NotifyFileWatcher::new()
        .watch_dir(root.clone(), tx)
        .expect("watch the root");

    fs::write(root.join("solo.yml"), b"processes: {}").expect("write direct child");
    let change = change_within(&mut rx, BUDGET).expect("a direct child change is reported");
    assert!(
        change.ends_with("solo.yml"),
        "expected the direct child, got {change:?}",
    );

    // Assert on the nested file's identity, not on silence: the direct-child write above
    // delivers its events asynchronously, so a late one can still be draining into the channel
    // when the nested write lands. A stray `solo.yml` event is fine — a `deep.rs` event is not.
    fs::write(nested.join("deep.rs"), b"//").expect("write nested file");
    let deadline = Instant::now() + QUIET;
    while Instant::now() < deadline {
        if let Ok(path) = rx.try_recv() {
            assert!(
                !path.ends_with("deep.rs"),
                "a non-recursive watch reported a nested change: {path:?}",
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
