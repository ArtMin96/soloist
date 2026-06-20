//! A [`PortProbe`] fake for scanner tests: it reports a fixed set of listening ports for
//! every group, without reading `/proc`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::portscan::PortProbe;
use crate::sync::lock;

/// An in-memory [`PortProbe`] that returns the same ports for every group it is asked about,
/// so the scanner's discovery/dedup/clear-on-stop and the readiness wait can be exercised
/// deterministically. The reported set is mutable ([`FakePortProbe::set`]) so a test can
/// flip a group from "not bound" to "bound" mid-wait.
#[derive(Clone)]
pub struct FakePortProbe {
    ports: Arc<Mutex<Vec<u16>>>,
}

impl FakePortProbe {
    /// A probe that reports `ports` for every requested group.
    pub fn returning(ports: Vec<u16>) -> Self {
        Self {
            ports: Arc::new(Mutex::new(ports)),
        }
    }

    /// Changes the ports the probe will report from now on (simulating a server binding).
    pub fn set(&self, ports: Vec<u16>) {
        *lock(&self.ports) = ports;
    }
}

impl PortProbe for FakePortProbe {
    fn listening_ports(&self, groups: &[i32]) -> HashMap<i32, Vec<u16>> {
        let ports = lock(&self.ports).clone();
        groups.iter().map(|&pgid| (pgid, ports.clone())).collect()
    }
}
