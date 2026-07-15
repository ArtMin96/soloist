//! Integration check against the real `/proc` port probe: it discovers a port a process in
//! the test's own group is listening on, and reports nothing for an absent group.

use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use soloist_core::PortProbe;
use soloist_sys::ProcPortProbe;

/// A pgid that will not exist on a normal Linux system.
const ABSENT_PGID: i32 = 999_999_999;

/// Spawns a child as its own process-group leader that holds `fd` (a listening socket) open past
/// `exec`, so `/proc/<child>/fd` exposes exactly that socket and the probe attributes its port to
/// the child's group. Clearing `FD_CLOEXEC` keeps the inherited socket open across the exec.
fn spawn_group_holding(fd: RawFd) -> std::process::Child {
    let mut command = Command::new("sleep");
    command.arg("30");
    // SAFETY: `fcntl` and `setpgid` are async-signal-safe and the only calls in the hook.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            libc::setpgid(0, 0);
            Ok(())
        });
    }
    command.spawn().expect("spawn a child process group")
}

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
fn ports_are_attributed_to_the_group_that_holds_them() {
    // Two concurrent live groups, each holding a distinct listening socket. The exact port number
    // is an unambiguous discriminator: a cross-attribution bug would credit one group with the
    // other's port.
    let listener_a = TcpListener::bind("127.0.0.1:0").expect("bind port a");
    let listener_b = TcpListener::bind("127.0.0.1:0").expect("bind port b");
    let port_a = listener_a.local_addr().expect("addr a").port();
    let port_b = listener_b.local_addr().expect("addr b").port();

    let mut child_a = spawn_group_holding(listener_a.as_raw_fd());
    let mut child_b = spawn_group_holding(listener_b.as_raw_fd());
    let pgid_a = child_a.id() as i32;
    let pgid_b = child_b.id() as i32;
    // Give the children a moment to reach exec and appear in /proc.
    sleep(Duration::from_millis(100));

    let probe = ProcPortProbe::new();
    let discovered = probe.listening_ports(&[pgid_a, pgid_b]);
    let ports_a = discovered.get(&pgid_a).expect("group a is present");
    let ports_b = discovered.get(&pgid_b).expect("group b is present");

    assert!(
        ports_a.contains(&port_a),
        "group a holds its own port {port_a}: {ports_a:?}"
    );
    assert!(
        !ports_a.contains(&port_b),
        "group a must not be credited group b's port {port_b}: {ports_a:?}"
    );
    assert!(
        ports_b.contains(&port_b),
        "group b holds its own port {port_b}: {ports_b:?}"
    );
    assert!(
        !ports_b.contains(&port_a),
        "group b must not be credited group a's port {port_a}: {ports_b:?}"
    );

    let _ = child_a.kill();
    let _ = child_a.wait();
    let _ = child_b.kill();
    let _ = child_b.wait();
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
