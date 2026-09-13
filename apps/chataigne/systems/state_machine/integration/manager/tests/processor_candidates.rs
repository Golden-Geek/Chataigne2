use std::collections::{HashMap, HashSet};

use golden_core::node::NodeId;

use super::super::ordered_active_processor_candidates;

#[test]
fn sparse_candidates_preserve_active_order_without_visiting_the_full_graph() {
    let active = (0..10_000).map(NodeId).collect::<Vec<_>>();
    let order = active
        .iter()
        .copied()
        .enumerate()
        .map(|(index, node)| (node, index))
        .collect::<HashMap<_, _>>();
    let candidates = HashSet::from([NodeId(9_000), NodeId(7), NodeId(20_000)]);

    assert_eq!(
        ordered_active_processor_candidates(&active, &order, Some(candidates)),
        vec![NodeId(7), NodeId(9_000)]
    );
    assert!(ordered_active_processor_candidates(&active, &order, Some(HashSet::new())).is_empty());
    assert_eq!(ordered_active_processor_candidates(&active, &order, None), active);
}
