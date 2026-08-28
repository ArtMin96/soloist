//! Integration check against the real `notify` file watcher: a file created, changed, or removed
//! under a watched root is reported on the change channel, dropping the handle stops the watch, and
//! a root the OS will not watch comes back as a refusal rather than as a handle that stays quiet.
//! The mock-clock matching/debounce behaviour is covered in the core; this is where which *kinds* of
//! filesystem event reach the core at all is pinned, since that is the one decision this adapter
//! makes. Uses real time (a short poll budget), like the other OS-adapter integration tests.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

use soloist_core::filewatch::{FileChange, FileChangeKind};
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

/// Blocks until a value arrives on `rx` or `budget` elapses, returning it if seen. Generic over
/// what the channel carries so both the legacy `PathBuf` port and the session-based `FileChange`
/// port share one polling loop.
fn change_within<T>(rx: &mut mpsc::Receiver<T>, budget: Duration) -> Option<T> {
    let deadline = Instant::now() + budget;
    loop {
        if let Ok(value) = rx.try_recv() {
            return Some(value);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Like [`change_within`], but skips any change whose path does not end in `suffix` rather than
/// returning it — a single write can raise more than one event (a create followed by a data
/// modification), so a test watching more than one file needs to wait for the one it asked about
/// rather than whichever arrives first.
fn change_ending_with(
    rx: &mut mpsc::Receiver<FileChange>,
    suffix: &str,
    budget: Duration,
) -> Option<FileChange> {
    let deadline = Instant::now() + budget;
    loop {
        if let Ok(change) = rx.try_recv() {
            if change.path.ends_with(suffix) {
                return Some(change);
            }
            continue;
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

// The tests below exercise the session-based port (`FileWatcher::open` / `WatchSession`), which
// backs many directories from one `notify` watcher instance rather than one instance per
// registration. The tests above cover the legacy `watch`/`watch_dir` path, which still ships
// alongside it.

#[test]
fn one_session_reports_from_two_sibling_directories() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    fs::create_dir_all(&first).expect("first dir");
    fs::create_dir_all(&second).expect("second dir");
    let (tx, mut rx) = mpsc::channel(64);

    let session = NotifyFileWatcher::new()
        .open(tx, Arc::new(AtomicU64::new(0)))
        .expect("open a session");
    session
        .watch_dir(&first)
        .expect("register the first directory");
    session
        .watch_dir(&second)
        .expect("register the second directory, on the same session");

    fs::write(first.join("a.txt"), b"a").expect("write into the first directory");
    let change = change_ending_with(&mut rx, "a.txt", BUDGET)
        .expect("a change under the first directory is reported");
    assert_eq!(change.kind, FileChangeKind::Appeared);

    fs::write(second.join("b.txt"), b"b").expect("write into the second directory");
    let change = change_ending_with(&mut rx, "b.txt", BUDGET)
        .expect("a change under the second directory is reported by the same session");
    assert_eq!(change.kind, FileChangeKind::Appeared);
}

#[test]
fn unwatching_one_directory_leaves_the_other_reporting() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    fs::create_dir_all(&first).expect("first dir");
    fs::create_dir_all(&second).expect("second dir");
    let (tx, mut rx) = mpsc::channel(64);

    let session = NotifyFileWatcher::new()
        .open(tx, Arc::new(AtomicU64::new(0)))
        .expect("open a session");
    session
        .watch_dir(&first)
        .expect("register the first directory");
    session
        .watch_dir(&second)
        .expect("register the second directory");

    session.unwatch(&first);
    // Give the backend a moment to drop the watch before changing the tree.
    std::thread::sleep(Duration::from_millis(100));
    while rx.try_recv().is_ok() {}

    fs::write(first.join("gone.txt"), b"x").expect("write into the unwatched directory");
    fs::write(second.join("still.txt"), b"y").expect("write into the still-watched directory");

    let change = change_within(&mut rx, BUDGET)
        .expect("the second directory still reports after the first was unwatched");
    assert!(
        change.path.ends_with("still.txt"),
        "expected the still-watched directory's file, got {:?}",
        change.path,
    );
}

#[test]
fn a_file_in_a_subdirectory_of_a_watched_directory_does_not_report() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().to_path_buf();
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("nested dir");
    let (tx, mut rx) = mpsc::channel(64);

    let session = NotifyFileWatcher::new()
        .open(tx, Arc::new(AtomicU64::new(0)))
        .expect("open a session");
    session
        .watch_dir(&root)
        .expect("register the root non-recursively");

    // The positive half, run first: a direct child is reported. Asserting the negative half
    // alone would pass just as well against a session delivering nothing at all.
    fs::write(root.join("direct.txt"), b"x").expect("write a direct child");
    let change = change_within(&mut rx, BUDGET).expect("a direct child change is reported");
    assert!(
        change.path.ends_with("direct.txt"),
        "expected the direct child, got {:?}",
        change.path,
    );
    assert_eq!(change.kind, FileChangeKind::Appeared);

    // The negative half: a file written a level deeper is not, because the registration is
    // non-recursive.
    fs::write(nested.join("deep.rs"), b"//").expect("write a nested file");
    let deadline = Instant::now() + QUIET;
    while Instant::now() < deadline {
        if let Ok(change) = rx.try_recv() {
            assert!(
                !change.path.ends_with("deep.rs"),
                "a non-recursive watch reported a nested change: {:?}",
                change.path,
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn capacity_reports_the_systems_watch_limit() {
    let capacity = NotifyFileWatcher::new().capacity();
    assert!(
        matches!(capacity, Some(n) if n > 0),
        "expected a positive watch capacity from /proc/sys/fs/inotify/max_user_watches, got {capacity:?}",
    );
}
