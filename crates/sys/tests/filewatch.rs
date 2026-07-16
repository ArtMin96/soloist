//! Integration check against the real `notify` file watcher: a file created under a watched
//! root is reported on the change channel, and dropping the handle stops the watch. The
//! mock-clock matching/debounce behaviour is covered in the core; this proves the OS watch
//! itself delivers create/modify paths. Uses real time (a short poll budget), like the other
//! OS-adapter integration tests.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use soloist_core::FileWatcher;
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
    let _handle = NotifyFileWatcher::new().watch(root.clone(), tx);

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

    let _handle = NotifyFileWatcher::new().watch(root.clone(), tx);

    let target = nested.join("main.rs");
    fs::write(&target, b"fn main() {}").expect("write nested file");

    let change = change_within(&mut rx, BUDGET).expect("a recursive watch reports a nested change");
    assert!(
        change.ends_with("main.rs"),
        "expected the nested file, got {change:?}",
    );
}

#[test]
fn dropping_the_handle_stops_the_watch() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let (tx, mut rx) = mpsc::channel(64);

    let handle = NotifyFileWatcher::new().watch(root.clone(), tx);
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
fn an_unwatchable_root_yields_no_events_rather_than_failing() {
    let missing = PathBuf::from("/nonexistent/soloist/watch/root");
    let (tx, mut rx) = mpsc::channel(64);

    // Best-effort: watching a path that does not exist returns a handle and simply reports
    // nothing, never panicking or failing the core.
    let _handle = NotifyFileWatcher::new().watch(missing, tx);

    assert!(
        rx.try_recv().is_err(),
        "an unwatchable root reports nothing"
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
    let _handle = NotifyFileWatcher::new().watch_dir(root.clone(), tx);

    fs::write(root.join("solo.yml"), b"processes: {}").expect("write direct child");
    let change = change_within(&mut rx, BUDGET).expect("a direct child change is reported");
    assert!(
        change.ends_with("solo.yml"),
        "expected the direct child, got {change:?}",
    );

    while rx.try_recv().is_ok() {}
    fs::write(nested.join("deep.rs"), b"//").expect("write nested file");
    assert!(
        change_within(&mut rx, QUIET).is_none(),
        "a nested change is not reported by a non-recursive watch",
    );
}
