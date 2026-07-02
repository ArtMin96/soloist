use tauri::async_runtime::JoinHandle;
use tokio::sync::oneshot;

use super::PtyBridge;

/// A forwarder stand-in that parks forever while holding `tx`; aborting the task drops the
/// sender, which is how a test observes the abort.
fn parked_forwarder() -> (JoinHandle<()>, oneshot::Receiver<()>) {
    let (tx, rx) = oneshot::channel::<()>();
    let handle = JoinHandle::Tokio(tokio::spawn(async move {
        let _keep = tx;
        std::future::pending::<()>().await
    }));
    (handle, rx)
}

/// Whether the task holding the paired sender has been dropped (i.e. aborted), giving the
/// scheduler time to process the cancellation.
async fn aborted(rx: &mut oneshot::Receiver<()>) -> bool {
    for _ in 0..64 {
        if let Err(oneshot::error::TryRecvError::Closed) = rx.try_recv() {
            return true;
        }
        tokio::task::yield_now().await;
    }
    false
}

#[tokio::test]
async fn install_returns_increasing_tokens() {
    let bridge = PtyBridge::default();
    let (first, _rx1) = parked_forwarder();
    let (second, _rx2) = parked_forwarder();
    let t1 = bridge.install(first);
    let t2 = bridge.install(second);
    assert!(t2 > t1);
}

#[tokio::test]
async fn installing_aborts_the_previous_forwarder() {
    let bridge = PtyBridge::default();
    let (first, mut rx1) = parked_forwarder();
    let (second, mut rx2) = parked_forwarder();
    bridge.install(first);
    bridge.install(second);
    assert!(aborted(&mut rx1).await);
    assert!(!aborted(&mut rx2).await);
}

#[tokio::test]
async fn a_stale_detach_keeps_the_newer_forwarder_running() {
    let bridge = PtyBridge::default();
    let (first, _rx1) = parked_forwarder();
    let (second, mut rx2) = parked_forwarder();
    let stale = bridge.install(first);
    bridge.install(second);
    bridge.clear(stale);
    assert!(!aborted(&mut rx2).await);
}

#[tokio::test]
async fn a_current_detach_stops_the_forwarder() {
    let bridge = PtyBridge::default();
    let (handle, mut rx) = parked_forwarder();
    let token = bridge.install(handle);
    bridge.clear(token);
    assert!(aborted(&mut rx).await);
}
