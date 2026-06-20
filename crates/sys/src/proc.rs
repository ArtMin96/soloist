//! Shared `/proc` reads used by the OS probes.
//!
//! Process-group membership is read from each task's process group (`/proc/<pid>/stat`),
//! which is exact: a descendant that reparents to init (a double-fork) keeps its group, where
//! a parent-tree walk would lose it. Both the port scanner and the metrics probe resolve a
//! managed process group this way, so the read lives here once.

use std::collections::HashMap;
use std::fs;

/// One `/proc` sweep into a process-group → member-pids map, reading each task's group from
/// `/proc/<pid>/stat`. A reparented descendant keeps its group, so it is still attributed to
/// the right one.
pub(crate) fn group_members() -> HashMap<i32, Vec<i32>> {
    let mut by_group: HashMap<i32, Vec<i32>> = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return by_group;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        if let Some(pgrp) = read_pgrp(pid) {
            by_group.entry(pgrp).or_default().push(pid);
        }
    }
    by_group
}

/// The process-group id of `pid` from `/proc/<pid>/stat`. The `comm` field can contain spaces
/// and parentheses, so the fixed fields are read after the final `)`: there they are
/// `state ppid pgrp …`, so `pgrp` is the third.
pub(crate) fn read_pgrp(pid: i32) -> Option<i32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    let mut fields = after_comm.split_whitespace();
    let _state = fields.next()?;
    let _ppid = fields.next()?;
    fields.next()?.parse::<i32>().ok()
}

/// The CPU time `pid` has consumed, in clock ticks (`utime + stime`) from `/proc/<pid>/stat`.
/// After the final `)` the numeric fields are `state ppid pgrp session tty_nr tpgid flags
/// minflt cminflt majflt cmajflt utime stime …`, so `utime`/`stime` are the 12th and 13th.
pub(crate) fn read_cpu_ticks(pid: i32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    Some(utime + stime)
}
