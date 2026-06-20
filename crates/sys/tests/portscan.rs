//! Integration check against the real `/proc` port probe: it discovers a port the test
//! process is actually listening on, and reports nothing for an absent group.

use std::collections::HashMap;
use std::net::TcpListener;

use soloist_core::PortProbe;
use soloist_sys::ProcPortProbe;

/// A pid that will not exist on a normal Linux system.
const ABSENT_PID: i32 = 999_999_999;

#[test]
fn discovers_a_port_the_test_process_is_listening_on() {
    // Bind a real listening socket; the test process now holds it open.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("local addr").port();

    let probe = ProcPortProbe::new();
    // Scan the test process's own subtree (itself as the group leader).
    let me = std::process::id() as i32;
    let discovered: HashMap<i32, Vec<u16>> = probe.listening_ports(&[me]);

    let ports = discovered.get(&me).expect("our own group is present");
    assert!(
        ports.contains(&port),
        "expected the bound port {port} in {ports:?}",
    );
}

#[test]
fn an_absent_group_has_no_ports() {
    let probe = ProcPortProbe::new();
    let discovered = probe.listening_ports(&[ABSENT_PID]);
    assert_eq!(discovered.get(&ABSENT_PID), Some(&Vec::new()));
}

#[test]
fn no_groups_means_no_readings() {
    let probe = ProcPortProbe::new();
    assert!(probe.listening_ports(&[]).is_empty());
}
