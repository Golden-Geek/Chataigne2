use super::*;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn snapshot_node(id: NodeId, uuid: NodeUuid, value: Option<ParamValue>) -> ProcessTreeNodeSnapshot {
    ProcessTreeNodeSnapshot {
        id,
        uuid,
        parent: None,
        first_child: None,
        next_sibling: None,
        node_type: "test".to_owned(),
        decl_id: format!("node_{}", id.0),
        short_name: String::new(),
        label: format!("Node {}", id.0),
        tags: Vec::new(),
        presentation: PresentationHint::default(),
        enabled: true,
        can_be_disabled: false,
        child_count: 0,
        param_value: value,
        param_constraints: None,
        param_control: None,
        dashboard_widget_target: DashboardWidgetTargetDescriptor::inspector_only(),
        script_properties: HashMap::new(),
        script_methods: Vec::new(),
    }
}

#[test]
fn snapshot_uuid_index_resolves_large_node_set() {
    const NODE_COUNT: u64 = 20_000;

    let mut nodes = HashMap::with_capacity(NODE_COUNT as usize);
    let mut expected = Vec::with_capacity(NODE_COUNT as usize);
    for value in 0..NODE_COUNT {
        let node_id = NodeId(value + 1);
        let uuid = NodeUuid(Uuid::from_u128(u128::from(value) + 1));
        nodes.insert(node_id, snapshot_node(node_id, uuid, None));
        expected.push((uuid, node_id));
    }

    let snapshot = ProcessTreeSnapshot::new(NodeId(1), nodes);

    for (uuid, node_id) in expected {
        assert_eq!(snapshot.node_id_by_uuid(uuid), Some(node_id));
    }
    assert_eq!(snapshot.node_id_by_uuid(NodeUuid::nil()), None);
}

#[test]
fn cloned_snapshot_with_param_values_preserves_uuid_index() {
    let node_id = NodeId(7);
    let uuid = NodeUuid(Uuid::from_u128(42));
    let snapshot = ProcessTreeSnapshot::new(
        node_id,
        HashMap::from([(node_id, snapshot_node(node_id, uuid, Some(ParamValue::Int(1))))]),
    );

    let updated = snapshot.with_param_values([(node_id, ParamValue::Int(2))]);

    assert_eq!(updated.node_id_by_uuid(uuid), Some(node_id));
    assert_eq!(
        updated.node(node_id).and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Int(2))
    );
}

#[test]
fn snapshot_indexes_children_in_sibling_order_and_preserves_first_decl_match() {
    let root = NodeId(1);
    let first = NodeId(2);
    let second = NodeId(3);
    let third = NodeId(4);

    let mut root_node = snapshot_node(root, NodeUuid(Uuid::from_u128(1)), None);
    root_node.first_child = Some(first);
    root_node.child_count = 3;

    let mut first_node = snapshot_node(first, NodeUuid(Uuid::from_u128(2)), None);
    first_node.parent = Some(root);
    first_node.next_sibling = Some(second);
    first_node.decl_id = "inputs/threshold".to_owned();

    let mut second_node = snapshot_node(second, NodeUuid(Uuid::from_u128(3)), None);
    second_node.parent = Some(root);
    second_node.next_sibling = Some(third);
    second_node.decl_id = "threshold".to_owned();

    let mut third_node = snapshot_node(third, NodeUuid(Uuid::from_u128(4)), None);
    third_node.parent = Some(root);
    third_node.next_sibling = Some(first);
    third_node.decl_id = "outputs/value".to_owned();

    let snapshot = ProcessTreeSnapshot::new(
        root,
        HashMap::from([
            (root, root_node),
            (first, first_node),
            (second, second_node),
            (third, third_node),
        ]),
    );

    assert_eq!(snapshot.child_ids_slice(root), &[first, second, third]);
    assert_eq!(snapshot.child_ids(root), vec![first, second, third]);
    assert_eq!(snapshot.child_at(root, 1), Some(second));
    assert_eq!(snapshot.previous_sibling(root, first), None);
    assert_eq!(snapshot.previous_sibling(root, third), Some(second));
    assert_eq!(snapshot.find_child_by_decl_id(root, "inputs/threshold"), Some(first));
    assert_eq!(
        snapshot.find_child_by_decl_id(root, "threshold"),
        Some(first),
        "the first sibling match must win even when a later decl id is exact"
    );
    assert_eq!(snapshot.find_child_by_decl_id(root, "value"), Some(third));
}

#[test]
fn snapshot_child_indexes_keep_twenty_thousand_sibling_lookups_linear() {
    const CHILD_COUNT: u64 = 20_000;
    const PERFORMANCE_BUDGET: Duration = Duration::from_secs(5);

    let root = NodeId(1);
    let mut root_node = snapshot_node(root, NodeUuid(Uuid::from_u128(1)), None);
    root_node.first_child = Some(NodeId(2));
    root_node.child_count = CHILD_COUNT as usize;

    let mut nodes = HashMap::with_capacity(CHILD_COUNT as usize + 1);
    nodes.insert(root, root_node);
    for index in 0..CHILD_COUNT {
        let node_id = NodeId(index + 2);
        let mut node = snapshot_node(node_id, NodeUuid(Uuid::from_u128(u128::from(index) + 2)), None);
        node.parent = Some(root);
        node.next_sibling = (index + 1 < CHILD_COUNT).then_some(NodeId(node_id.0 + 1));
        node.decl_id = format!("lanes/lane_{index}");
        nodes.insert(node_id, node);
    }

    let build_started = Instant::now();
    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let build_elapsed = build_started.elapsed();

    assert_eq!(snapshot.child_ids_slice(root).len(), CHILD_COUNT as usize);
    assert!(
        build_elapsed < PERFORMANCE_BUDGET,
        "20k-node child indexes took {build_elapsed:?} to build"
    );

    let lookup_started = Instant::now();
    for index in (0..CHILD_COUNT).rev() {
        assert_eq!(
            snapshot.find_child_by_decl_id(root, &format!("lane_{index}")),
            Some(NodeId(index + 2))
        );
    }
    let lookup_elapsed = lookup_started.elapsed();
    assert!(
        lookup_elapsed < PERFORMANCE_BUDGET,
        "20k indexed declaration lookups took {lookup_elapsed:?}"
    );
}

#[test]
fn duplicate_uuid_lookup_preserves_first_snapshot_match() {
    let duplicate_uuid = NodeUuid(Uuid::from_u128(42));
    let first_id = NodeId(1);
    let second_id = NodeId(2);
    let nodes = HashMap::from([
        (first_id, snapshot_node(first_id, duplicate_uuid, None)),
        (second_id, snapshot_node(second_id, duplicate_uuid, None)),
    ]);
    let expected = nodes
        .iter()
        .find_map(|(node_id, node)| (node.uuid == duplicate_uuid).then_some(*node_id));

    let snapshot = ProcessTreeSnapshot::new(first_id, nodes);

    assert_eq!(snapshot.node_id_by_uuid(duplicate_uuid), expected);
}
