//! A [`PortProbe`] fake for scanner tests: it reports a fixed set of listening ports for
//! every group, without reading `/proc`.

use std::collections::HashMap;

use crate::portscan::PortProbe;

/// An in-memory [`PortProbe`] that returns the same ports for every group it is asked about,
/// so the scanner's discovery, change-dedup, and clear-on-stop behaviour can be exercised
/// deterministically.
#[derive(Clone)]
pub struct FakePortProbe {
    ports: Vec<u16>,
}

impl FakePortProbe {
    /// A probe that reports `ports` for every requested group.
    pub fn returning(ports: Vec<u16>) -> Self {
        Self { ports }
    }
}

impl PortProbe for FakePortProbe {
    fn listening_ports(&self, groups: &[i32]) -> HashMap<i32, Vec<u16>> {
        groups
            .iter()
            .map(|&pgid| (pgid, self.ports.clone()))
            .collect()
    }
}
