//! Behavioural tests for [`PortScanner`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes and the mock clock, so timing is deterministic with no
//! real time and no real `/proc` read.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::portscan::test_support::{
    running_process, setup, view_of, wait_for_status, ADVANCE_STEP,
};
use crate::process::ProcStatus;
use crate::testing::{FakePortProbe, MockClock};

use super::PortScanner;

/// Advances the mock clock and yields until a `PortsChanged` for `id` arrives, or fails
/// after a bounded number of rounds. Each round fires the pending scan timer and lets the
/// spawned tasks progress, so the scanner is driven deterministically with no real time.
async fn next_ports_changed(
    rx: &mut broadcast::Receiver<DomainEvent>,
    clock: &MockClock,
    id: ProcessId,
) -> Vec<u16> {
    for _ in 0..200 {
        clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        while let Ok(event) = rx.try_recv() {
            if let DomainEvent::PortsChanged { id: got, ports } = event {
                if got == id {
                    return ports;
                }
            }
        }
    }
    panic!("no PortsChanged for {id:?} within the budget");
}

#[tokio::test]
async fn a_running_process_has_its_ports_discovered_then_announced_once() {
    let mut s = setup();
    let id = running_process(&mut s).await;

    let probe = FakePortProbe::returning(vec![8080]);
    tokio::spawn(
        PortScanner::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );

    // Discovery announces the port and reflects it on the read model.
    assert_eq!(
        next_ports_changed(&mut s.rx, &s.clock, id).await,
        vec![8080]
    );
    assert_eq!(view_of(&s.sup, id).ports, vec![8080]);

    // A later scan with the same ports announces nothing — the read model never churns.
    for _ in 0..3 {
        s.clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
    }
    let mut churned = false;
    while let Ok(event) = s.rx.try_recv() {
        if matches!(event, DomainEvent::PortsChanged { id: got, .. } if got == id) {
            churned = true;
        }
    }
    assert!(
        !churned,
        "an unchanged scan announces no further PortsChanged"
    );
}

#[tokio::test]
async fn ports_clear_when_the_process_stops() {
    let mut s = setup();
    let id = running_process(&mut s).await;

    let probe = FakePortProbe::returning(vec![5173]);
    tokio::spawn(
        PortScanner::new(
            Arc::new(s.clock.clone()),
            Arc::new(probe),
            s.bus.clone(),
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
    assert_eq!(
        next_ports_changed(&mut s.rx, &s.clock, id).await,
        vec![5173]
    );

    // Stopping the process ends its group, so its discovered ports are cleared.
    s.sup.stop(id);
    wait_for_status(&mut s.rx, id, ProcStatus::Stopped).await;
    assert!(
        view_of(&s.sup, id).ports.is_empty(),
        "a stopped process lists no ports",
    );
}
