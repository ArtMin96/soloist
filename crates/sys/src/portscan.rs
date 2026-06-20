//! Port discovery over `/proc`: the OS read behind the core's `PortProbe`.
//!
//! For each requested group (its leader pid), it walks the process subtree, collects the
//! socket inodes those processes hold open (`/proc/<pid>/fd/*` → `socket:[inode]`), and
//! joins them to the LISTEN-state entries in `/proc/net/tcp{,6}` to recover the bound
//! ports. The `/proc` snapshot (process tree + listening sockets) is read **once per call**
//! and reused across every group, so a scan tick costs a single sweep.

use std::collections::{HashMap, HashSet};
use std::fs;

use soloist_core::PortProbe;

/// The `/proc/net/tcp{,6}` connection-state code for a listening socket.
const TCP_LISTEN: &str = "0A";

/// Discovers listening TCP ports per process group by reading `/proc`.
#[derive(Clone, Copy, Default)]
pub struct ProcPortProbe;

impl ProcPortProbe {
    pub fn new() -> Self {
        Self
    }
}

impl PortProbe for ProcPortProbe {
    fn listening_ports(&self, groups: &[i32]) -> HashMap<i32, Vec<u16>> {
        if groups.is_empty() {
            return HashMap::new();
        }
        // Read the process tree and the listening-socket table once, then resolve each group
        // against them.
        let (live, children) = process_tree();
        let ports_by_inode = listening_ports_by_inode();
        groups
            .iter()
            .map(|&leader| {
                let pids = subtree(&live, &children, leader);
                (leader, ports_for_pids(&pids, &ports_by_inode))
            })
            .collect()
    }
}

/// Reads `/proc` once into the set of live pids and a parent → children adjacency map.
fn process_tree() -> (HashSet<i32>, HashMap<i32, Vec<i32>>) {
    let mut live = HashSet::new();
    let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return (live, children);
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        live.insert(pid);
        if let Some(ppid) = read_ppid(pid) {
            children.entry(ppid).or_default().push(pid);
        }
    }
    (live, children)
}

/// The parent pid of `pid` from `/proc/<pid>/stat`. The `comm` field can contain spaces and
/// parentheses, so fields are read after the final `)`: there they are `state ppid pgrp …`.
fn read_ppid(pid: i32) -> Option<i32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    let mut fields = after_comm.split_whitespace();
    let _state = fields.next()?;
    fields.next()?.parse::<i32>().ok()
}

/// The set of pids in the subtree rooted at `leader` (inclusive), or empty if the leader is
/// no longer live (the group has exited).
fn subtree(live: &HashSet<i32>, children: &HashMap<i32, Vec<i32>>, leader: i32) -> HashSet<i32> {
    if !live.contains(&leader) {
        return HashSet::new();
    }
    let mut seen = HashSet::new();
    let mut stack = vec![leader];
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    seen
}

/// The sorted, de-duplicated ports the given pids' open sockets are listening on.
fn ports_for_pids(pids: &HashSet<i32>, ports_by_inode: &HashMap<u64, u16>) -> Vec<u16> {
    let mut ports: Vec<u16> = pids
        .iter()
        .flat_map(|&pid| socket_inodes(pid))
        .filter_map(|inode| ports_by_inode.get(&inode).copied())
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// The socket inodes a process holds open, from its `/proc/<pid>/fd/*` symlinks (each a
/// `socket:[inode]` target). Unreadable entries are skipped.
fn socket_inodes(pid: i32) -> Vec<u64> {
    let Ok(fds) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return Vec::new();
    };
    fds.flatten()
        .filter_map(|fd| fs::read_link(fd.path()).ok())
        .filter_map(|target| parse_socket_inode(&target.to_string_lossy()))
        .collect()
}

/// Extracts the inode from a `socket:[12345]` symlink target.
fn parse_socket_inode(link: &str) -> Option<u64> {
    link.strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse::<u64>()
        .ok()
}

/// Maps each LISTEN-state socket inode to its local port, from `/proc/net/tcp` and `tcp6`.
fn listening_ports_by_inode() -> HashMap<u64, u16> {
    let mut map = HashMap::new();
    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        // Columns: sl local_address rem_address st … (uid timeout) inode …
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 || fields[3] != TCP_LISTEN {
                continue;
            }
            let Some(port) = fields[1]
                .rsplit_once(':')
                .and_then(|(_, hex)| u16::from_str_radix(hex, 16).ok())
            else {
                continue;
            };
            let Ok(inode) = fields[9].parse::<u64>() else {
                continue;
            };
            map.insert(inode, port);
        }
    }
    map
}
