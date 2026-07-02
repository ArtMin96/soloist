use std::collections::HashSet;

use super::AgentLineage;
use crate::ids::ProcessId;

#[test]
fn parent_of_returns_the_recorded_parent() {
    let lineage = AgentLineage::new();
    let lead = ProcessId::next();
    let worker = ProcessId::next();
    lineage.record(worker, lead);
    assert_eq!(lineage.parent_of(worker), Some(lead));
}

#[test]
fn an_unrecorded_process_has_no_parent() {
    let lineage = AgentLineage::new();
    assert_eq!(lineage.parent_of(ProcessId::next()), None);
}

#[test]
fn edges_returns_every_recorded_pair_sorted_by_child() {
    let lineage = AgentLineage::new();
    let lead = ProcessId::next();
    let first_worker = ProcessId::next();
    let second_worker = ProcessId::next();
    // Recorded out of id order to prove the read sorts by child.
    lineage.record(second_worker, lead);
    lineage.record(first_worker, lead);

    assert_eq!(
        lineage.edges(),
        vec![(first_worker, lead), (second_worker, lead)],
    );
}

#[test]
fn retain_live_drops_children_gone_from_the_registry() {
    let lineage = AgentLineage::new();
    let lead = ProcessId::next();
    let live_worker = ProcessId::next();
    let gone_worker = ProcessId::next();
    lineage.record(live_worker, lead);
    lineage.record(gone_worker, lead);

    lineage.retain_live(&HashSet::from([lead, live_worker]));

    assert_eq!(lineage.parent_of(live_worker), Some(lead));
    assert_eq!(lineage.parent_of(gone_worker), None);
}
