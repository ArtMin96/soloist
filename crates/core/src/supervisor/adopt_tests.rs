//! The adopted-group signal guard: a stop or kill signals the group only while the
//! recorded identity still matches, so a pgid the OS reassigned to an unrelated group
//! after the adopted process died is never signalled.

use std::path::PathBuf;
use std::sync::Arc;

use super::GroupSignal;
use crate::ports::{OrphanRecord, ProcessControl, ProcessIdentity};
use crate::testing::{fake_identity, FakeOrphanControl};

fn record(pgid: i32) -> OrphanRecord {
    OrphanRecord {
        project_root: PathBuf::from("/p"),
        name: "web".into(),
        command: "npm run dev".into(),
        pgid,
        identity: Some(fake_identity()),
    }
}

#[tokio::test]
async fn signals_the_group_while_its_identity_matches() {
    let control = Arc::new(FakeOrphanControl::new());
    control.set_alive(555);
    let mut signal = GroupSignal {
        record: record(555),
        control: control.clone(),
    };

    signal.kill().await.expect("kill");

    assert!(
        control.signalled().contains(&(555, true)),
        "a matching group is SIGKILLed"
    );
}

#[tokio::test]
async fn does_not_signal_a_recycled_group() {
    let control = Arc::new(FakeOrphanControl::new());
    // pgid 555 is now an unrelated group (a different boot), not the adopted process.
    control.set_identity(
        555,
        ProcessIdentity {
            boot_id: "boot-other".into(),
            started_at: fake_identity().started_at,
        },
    );
    let mut signal = GroupSignal {
        record: record(555),
        control: control.clone(),
    };

    signal.terminate().await.expect("terminate is a no-op");
    signal.kill().await.expect("kill is a no-op");

    assert!(
        control.signalled().is_empty(),
        "a recycled group is never signalled"
    );
}
