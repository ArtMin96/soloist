use tauri::async_runtime::JoinHandle;
use tokio::sync::oneshot;

use super::{PtyBridge, MAX_FORWARDERS};

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
async fn installing_another_forwarder_keeps_the_previous_running() {
    // The terminal pool streams several processes at once, so a second install does not disturb
    // the first — both forwarders run until each is cleared by its own token.
    let bridge = PtyBridge::default();
    let (first, mut rx1) = parked_forwarder();
    let (second, mut rx2) = parked_forwarder();
    bridge.install(first);
    bridge.install(second);
    assert!(!aborted(&mut rx1).await);
    assert!(!aborted(&mut rx2).await);
}

#[tokio::test]
async fn clearing_one_forwarder_leaves_the_others_running() {
    let bridge = PtyBridge::default();
    let (first, mut rx1) = parked_forwarder();
    let (second, mut rx2) = parked_forwarder();
    let first_token = bridge.install(first);
    bridge.install(second);
    bridge.clear(first_token);
    assert!(aborted(&mut rx1).await, "the cleared forwarder stops");
    assert!(!aborted(&mut rx2).await, "the other keeps running");
}

#[tokio::test]
async fn a_current_detach_stops_the_forwarder() {
    let bridge = PtyBridge::default();
    let (handle, mut rx) = parked_forwarder();
    let token = bridge.install(handle);
    bridge.clear(token);
    assert!(aborted(&mut rx).await);
}

#[tokio::test]
async fn installing_past_the_ceiling_reclaims_the_oldest_forwarder() {
    // A webview reload or crash loses every detach token, so without a ceiling the orphaned
    // forwarders would accumulate forever. Installing past the ceiling aborts the oldest — an
    // orphan, since a shown pane holds a newer token — so the map stays bounded.
    let bridge = PtyBridge::default();
    let mut parked = Vec::new();
    for _ in 0..MAX_FORWARDERS {
        let (handle, rx) = parked_forwarder();
        bridge.install(handle);
        parked.push(rx);
    }
    let (extra, mut extra_rx) = parked_forwarder();
    bridge.install(extra);

    assert!(
        aborted(&mut parked[0]).await,
        "the oldest forwarder is reclaimed"
    );
    assert!(
        !aborted(&mut parked[1]).await,
        "a newer forwarder keeps running"
    );
    assert!(
        !aborted(&mut extra_rx).await,
        "the just-installed forwarder keeps running"
    );
}
