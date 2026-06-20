//! Integration check against the real `/proc` port probe: it discovers a port a process in
//! the test's own group is listening on, and reports nothing for an absent group.

use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;

use soloist_core::PortProbe;
use soloist_sys::ProcPortProbe;

/// A pgid that will not exist on a normal Linux system.
const ABSENT_PGID: i32 = 999_999_999;

/// This process's own process-group id, read from `/proc/self/stat` (the fields after the
/// final `)` are `state ppid pgrp …`). The probe groups by pgid, so the test asks about the
/// group it actually belongs to rather than assuming it leads its own group.
fn current_pgid() -> i32 {
    let stat = fs::read_to_string("/proc/self/stat").expect("read /proc/self/stat");
    let after_comm = stat.rsplit_once(')').expect("stat has a comm field").1;
    let mut fields = after_comm.split_whitespace();
    let _state = fields.next();
    let _ppid = fields.next();
    fields
        .next()
        .and_then(|pgrp| pgrp.parse::<i32>().ok())
        .expect("pgrp field is an integer")
}

#[test]
fn discovers_a_port_a_process_in_the_group_is_listening_on() {
    // Bind a real listening socket; the test process now holds it open.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("local addr").port();

    let probe = ProcPortProbe::new();
    let pgid = current_pgid();
    let discovered: HashMap<i32, Vec<u16>> = probe.listening_ports(&[pgid]);

    let ports = discovered.get(&pgid).expect("our own group is present");
    assert!(
        ports.contains(&port),
        "expected the bound port {port} in {ports:?}",
    );
}

#[test]
fn an_absent_group_has_no_ports() {
    let probe = ProcPortProbe::new();
    let discovered = probe.listening_ports(&[ABSENT_PGID]);
    assert_eq!(discovered.get(&ABSENT_PGID), Some(&Vec::new()));
}

#[test]
fn no_groups_means_no_readings() {
    let probe = ProcPortProbe::new();
    assert!(probe.listening_ports(&[]).is_empty());
}
