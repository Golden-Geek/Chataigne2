use std::collections::HashMap;

use golden_core::node::NodeId;

use super::reconciled_state_activity;

fn activity(result: &HashMap<NodeId, bool>, id: u64) -> bool {
    result.get(&NodeId(id)).copied().unwrap_or(false)
}

#[test]
fn disconnected_networks_each_keep_one_active_state() {
    let states = [(NodeId(1), true), (NodeId(2), true), (NodeId(3), false)];
    let transitions = [(NodeId(2), NodeId(3))];

    let result = reconciled_state_activity(&states, &transitions, None, None);

    assert!(activity(&result, 1));
    assert!(activity(&result, 2));
    assert!(!activity(&result, 3));
}

#[test]
fn connecting_active_networks_keeps_exactly_one_active_state() {
    let states = [(NodeId(1), true), (NodeId(2), true)];
    let transitions = [(NodeId(1), NodeId(2))];

    let result = reconciled_state_activity(&states, &transitions, None, None);

    assert!(activity(&result, 1));
    assert!(!activity(&result, 2));
}

#[test]
fn activating_a_state_switches_its_whole_network() {
    let states = [(NodeId(1), true), (NodeId(2), false), (NodeId(3), true)];
    let transitions = [(NodeId(1), NodeId(2))];

    let result = reconciled_state_activity(&states, &transitions, Some(NodeId(2)), None);

    assert!(!activity(&result, 1));
    assert!(activity(&result, 2));
    assert!(activity(&result, 3));
}

#[test]
fn deactivating_the_active_state_promotes_another_network_member() {
    let states = [(NodeId(1), false), (NodeId(2), false)];
    let transitions = [(NodeId(1), NodeId(2))];

    let result = reconciled_state_activity(&states, &transitions, None, Some(NodeId(1)));

    assert!(!activity(&result, 1));
    assert!(activity(&result, 2));
}
