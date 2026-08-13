use std::collections::HashSet;

use super::AgentLineage;
use crate::ids::ProcessId;

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

#[test]
fn retain_live_keeps_only_dead_ancestors_connecting_live_descendants() {
    let lineage = AgentLineage::new();
    let dead_lead = ProcessId::next();
    let dead_middle = ProcessId::next();
    let live_worker = ProcessId::next();
    let dead_group_lead = ProcessId::next();
    let dead_group_worker = ProcessId::next();
    lineage.record(dead_middle, dead_lead);
    lineage.record(live_worker, dead_middle);
    lineage.record(dead_group_worker, dead_group_lead);

    lineage.retain_live(&HashSet::from([live_worker]));

    assert_eq!(lineage.root_of(live_worker), dead_lead);
    assert_eq!(lineage.parent_of(dead_middle), Some(dead_lead));
    assert_eq!(lineage.parent_of(dead_group_worker), None);
}

#[test]
fn repeated_dead_groups_are_fully_pruned() {
    let lineage = AgentLineage::new();
    for _ in 0..128 {
        lineage.record(ProcessId::next(), ProcessId::next());
    }

    lineage.retain_live(&HashSet::new());

    assert!(lineage.edges().is_empty());
}
