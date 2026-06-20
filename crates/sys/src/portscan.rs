//! Port discovery over `/proc`: the OS read behind the core's `PortProbe`.
//!
//! For each requested group (its leader pid, which is the group's pgid), it finds the
//! processes whose process group is that pgid, collects the socket inodes those processes
//! hold open (`/proc/<pid>/fd/*` → `socket:[inode]`), and joins them to the LISTEN-state
//! entries in `/proc/net/tcp{,6}` to recover the bound ports. Membership is read straight
//! from each task's process group (`/proc/<pid>/stat`), so a descendant that reparents to
//! init is still counted — unlike a parent-tree walk, which it would escape. The `/proc`
//! snapshot (group membership + listening sockets) is read **once per call** and reused
//! across every group, so a scan tick costs a single sweep.

use std::collections::HashMap;
use std::fs;

use soloist_core::PortProbe;

use crate::proc::group_members;

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
        // Read group membership and the listening-socket table once, then resolve each
        // requested group against them.
        let members_by_group = group_members();
        let ports_by_inode = listening_ports_by_inode();
        groups
            .iter()
            .map(|&pgid| {
                let pids = members_by_group
                    .get(&pgid)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                (pgid, ports_for_pids(pids, &ports_by_inode))
            })
            .collect()
    }
}

/// The sorted, de-duplicated ports the given pids' open sockets are listening on.
fn ports_for_pids(pids: &[i32], ports_by_inode: &HashMap<u64, u16>) -> Vec<u16> {
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
