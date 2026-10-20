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
        snapshot.node(node_id).and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Int(1)),
        "overlaying a value must leave the retained snapshot unchanged"
    );
    assert_eq!(
        updated.node(node_id).and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Int(2))
    );

    let updated_again = updated.with_param_values([(node_id, ParamValue::Int(3)), (NodeId(999), ParamValue::Int(4))]);
    assert_eq!(
        updated_again.node(node_id).and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Int(3))
    );
    assert_eq!(
        updated.node(node_id).and_then(|node| node.param_value.as_ref()),
        Some(&ParamValue::Int(2))
    );
    assert!(updated_again.node(NodeId(999)).is_none());
}

#[test]
fn deep_subtree_revisions_fold_iteratively_and_do_not_rewrite_a_retained_view() {
    let mut clock = super::super::tree_revisions::TreeRevisions::default();
    let mut nodes = HashMap::new();
    for value in 1..=20_000 {
        let id = NodeId(value);
        let mut node = snapshot_node(id, NodeUuid(Uuid::from_u128(u128::from(value))), None);
        node.parent = (value > 1).then_some(NodeId(value - 1));
        node.first_child = (value < 20_000).then_some(NodeId(value + 1));
        nodes.insert(id, node);
    }
    let before = ProcessTreeSnapshot::from_indexed_nodes(NodeId(1), nodes.clone(), HashMap::new(), clock.snapshot());
    let change = clock.next_change();
    clock.mark(NodeId(20_000), change);
    let after = ProcessTreeSnapshot::from_indexed_nodes(NodeId(1), nodes, HashMap::new(), clock.snapshot());
    assert_ne!(before.subtree_revision(NodeId(1)), after.subtree_revision(NodeId(1)));
    assert_eq!(
        after.subtree_revision(NodeId(1)),
        after.subtree_revision(NodeId(20_000))
    );
    assert_ne!(
        before.subtree_revision(NodeId(20_000)),
        after.subtree_revision(NodeId(20_000))
    );
}

#[test]
fn snapshot_reservations_include_spare_dynamic_capacity_and_owned_overlays() {
    let id = NodeId(1);
    let plain = snapshot_node(id, NodeUuid(Uuid::from_u128(1)), Some(ParamValue::Int(0)));
    let base = ProcessTreeSnapshot::new(id, HashMap::from([(id, plain.clone())]));
    let mut large = plain;
    let mut text = String::with_capacity(1024 * 1024);
    text.push('x');
    large.param_value = Some(ParamValue::Str(text));
    large.presentation.icon = Some(String::with_capacity(32 * 1024));
    large.script_methods = Vec::with_capacity(1000);
    let snapshot = ProcessTreeSnapshot::new(id, HashMap::from([(id, large)]));
    assert!(snapshot.retained_bytes() >= base.retained_bytes() + 1024 * 1024 + 32 * 1024);
    let bytes = snapshot.retained_bytes();
    let overlay = snapshot.with_param_values([(id, ParamValue::Str("z".repeat(512 * 1024)))]);
    assert!(overlay.retained_bytes() >= bytes + 512 * 1024);
    assert_eq!(snapshot.retained_bytes(), bytes);
    assert_eq!(
        snapshot.clone().retained_bytes(),
        bytes,
        "shared storage is charged without cloning its nodes"
    );
    let cloned_overlay = overlay.clone();
    assert!(cloned_overlay.retained_bytes() >= bytes + 512 * 1024);
    assert!(
        std::ptr::eq(overlay.node(id).unwrap(), cloned_overlay.node(id).unwrap()),
        "cloned views share immutable overlay records"
    );
}

#[test]
fn snapshot_leases_share_identity_and_large_views_have_an_explicit_byte_budget() {
    let mut nodes = HashMap::new();
    for value in 1..=50_000 {
        let id = NodeId(value);
        nodes.insert(
            id,
            snapshot_node(id, NodeUuid(Uuid::from_u128(u128::from(value))), None),
        );
    }
    let snapshot = Arc::new(ProcessTreeSnapshot::new(NodeId(1), nodes));
    assert!(
        snapshot.retained_bytes() < 1024 * 1024 * 1024,
        "a 50k-record view should fit the canonical lease budget"
    );
    let first = snapshot.try_lease().unwrap();
    let second = snapshot.try_lease().unwrap();
    assert!(std::ptr::eq(&*first, &*second));
    assert!(std::ptr::eq(&*first, &*snapshot));
    assert_eq!(first.node_count(), 50_000);
}

#[test]
fn snapshot_reservations_cover_constraints_controls_references_and_presentation_allocations() {
    use crate::parameter::{
        ParameterConstraints, ParameterControlDiagnostic, ParameterControlMode, ParameterControlSpec,
        ParameterControlState, ParameterEnumOption, RangeConstraint, ReferenceRoot,
    };
    const CAPACITY: usize = 64 * 1024;
    let text = || {
        let mut value = String::with_capacity(CAPACITY);
        value.push('x');
        value
    };
    let id = NodeId(1);
    let plain = snapshot_node(id, NodeUuid(Uuid::from_u128(1)), None);
    let baseline = ProcessTreeSnapshot::new(id, HashMap::from([(id, plain.clone())]));
    let mut node = plain;
    node.node_type = text();
    node.decl_id = text();
    node.short_name = text();
    node.label = text();
    node.tags = vec![text()];
    node.presentation.icon = Some(text());
    node.presentation.warnings = vec![crate::node::NodeWarning {
        id: text(),
        message: text(),
        detail: Some(text()),
    }];
    node.dashboard_widget_target.default_widget_type_id = text();
    node.dashboard_widget_target.widget_types[0].id = text();
    node.dashboard_widget_target.widget_types[0].label = text();
    node.script_properties.insert(text(), ParamValue::Str(text()));
    node.script_methods = vec![text()];
    let mut reference = NodeReference::new(NodeUuid::nil());
    reference.cached_name = Some(text());
    reference.relative_path_from_root = vec![text()];
    let mut constraints = ParameterConstraints {
        enum_options: (vec![ParameterEnumOption {
            variant_id: text(),
            value: ParamValue::Reference(reference),
            label: text(),
            tags: vec![text()],
            ordering: None,
        }])
        .into(),
        ..ParameterConstraints::default()
    };
    constraints.reference.root = ReferenceRoot::RelativeToOwner { path: vec![text()] };
    constraints.reference.allowed_node_types = vec![text()];
    constraints.reference.allowed_parameter_types = vec![text()];
    constraints.reference.custom_filter_key = Some(text());
    constraints.reference.default_search_filter = Some(text());
    constraints.file.allowed_extensions = vec![text()];
    constraints.range = Some(RangeConstraint::Components {
        min: Some(Vec::with_capacity(CAPACITY / std::mem::size_of::<f64>())),
        max: Some(Vec::with_capacity(CAPACITY / std::mem::size_of::<f64>())),
    });
    node.param_constraints = Some(constraints);
    node.param_control = Some(ParameterControlState {
        mode: ParameterControlMode::TemplateText,
        spec: ParameterControlSpec::TemplateText { template: text() },
        diagnostics: vec![ParameterControlDiagnostic {
            code: text(),
            message: text(),
            detail: Some(text()),
        }],
    });
    let snapshot = ProcessTreeSnapshot::new(id, HashMap::from([(id, node)]));
    // 30 strings and two spare f64 buffers. Container/header slack cannot hide a missed
    // allocation of this size; subtract only the tiny baseline label/type allocations.
    assert!(snapshot.retained_bytes() >= baseline.retained_bytes() + 32 * CAPACITY - 128);
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
fn declaration_lookup_preserves_order_across_small_and_wide_parents() {
    for child_count in [16_u64, 17] {
        let root = NodeId(1);
        let mut root_node = snapshot_node(root, NodeUuid(Uuid::from_u128(1)), None);
        root_node.first_child = Some(NodeId(2));
        root_node.child_count = child_count as usize;
        let mut nodes = HashMap::from([(root, root_node)]);
        for index in 0..child_count {
            let id = NodeId(index + 2);
            let mut child = snapshot_node(id, NodeUuid(Uuid::from_u128(u128::from(id.0))), None);
            child.parent = Some(root);
            child.next_sibling = (index + 1 < child_count).then_some(NodeId(id.0 + 1));
            child.decl_id = match index {
                0 => "nested/match".to_owned(),
                1 => "match".to_owned(),
                _ => format!("child_{index}"),
            };
            nodes.insert(id, child);
        }

        let snapshot = ProcessTreeSnapshot::new(root, nodes);
        assert_eq!(snapshot.find_child_by_decl_id(root, "nested/match"), Some(NodeId(2)));
        assert_eq!(snapshot.find_child_by_decl_id(root, "match"), Some(NodeId(2)));
        assert_eq!(snapshot.find_child_by_decl_id(root, "child_2"), Some(NodeId(4)));
        assert_eq!(snapshot.find_child_by_decl_id(root, "missing"), None);
    }
}

#[test]
fn child_key_lookup_preserves_first_match_across_aliases_and_parent_widths() {
    for count in [16_u64, 17, 20_000] {
        let root = NodeId(1);
        let mut root_node = snapshot_node(root, NodeUuid(Uuid::from_u128(1)), None);
        root_node.first_child = Some(NodeId(2));
        let mut nodes = HashMap::from([(root, root_node)]);
        for index in 0..count {
            let id = NodeId(index + 2);
            let mut child = snapshot_node(id, NodeUuid(Uuid::from_u128(u128::from(id.0))), None);
            child.parent = Some(root);
            child.next_sibling = (index + 1 < count).then_some(NodeId(id.0 + 1));
            child.decl_id = format!("declaration_{index}");
            child.short_name = format!("short_{index}");
            child.label = format!("label_{index}");
            if index == 0 {
                child.label = "collision".into();
            }
            if index == 1 {
                child.decl_id = "collision".into();
            }
            nodes.insert(id, child);
        }
        let snapshot = ProcessTreeSnapshot::new(root, nodes);
        assert_eq!(snapshot.find_child(root, "collision"), Some(NodeId(2)));
        for index in (2..count).rev() {
            for key in [
                format!("declaration_{index}"),
                format!("short_{index}"),
                format!("label_{index}"),
            ] {
                assert_eq!(snapshot.find_child(root, &key), Some(NodeId(index + 2)));
            }
        }
        assert_eq!(snapshot.find_child(root, "absent"), None);
    }
}

#[test]
fn snapshot_child_index_stops_a_cycle_after_the_inline_sibling_limit() {
    let root = NodeId(1);
    let mut root_node = snapshot_node(root, NodeUuid(Uuid::from_u128(1)), None);
    root_node.first_child = Some(NodeId(2));
    root_node.child_count = 10;
    let mut nodes = HashMap::from([(root, root_node)]);
    for index in 0..10_u64 {
        let id = NodeId(index + 2);
        let mut node = snapshot_node(id, NodeUuid(Uuid::from_u128(u128::from(id.0))), None);
        node.parent = Some(root);
        node.next_sibling = Some(if index == 9 { NodeId(5) } else { NodeId(id.0 + 1) });
        nodes.insert(id, node);
    }

    let snapshot = ProcessTreeSnapshot::new(root, nodes);
    let expected = (2..12).map(NodeId).collect::<Vec<_>>();
    assert_eq!(snapshot.child_ids_slice(root), expected.as_slice());
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

#[test]
fn large_tick_snapshot_retirement_is_bounded() {
    let make_snapshot = || {
        let mut nodes = HashMap::with_capacity(10_000);
        for value in 1..=10_000_u64 {
            let node_id = NodeId(value);
            nodes.insert(
                node_id,
                snapshot_node(node_id, NodeUuid(Uuid::from_u128(u128::from(value))), None),
            );
        }
        Arc::new(ProcessTreeSnapshot::new(NodeId(1), nodes))
    };
    let mut engine = Engine::new(Folder::new("root"));
    engine.tick_tree_snapshot = Some(make_snapshot());
    engine.clear_tick_tree_snapshot();
    assert!(engine.snapshot_retirements.wait_for_idle(Duration::from_secs(5)));
    assert_eq!(engine.process_snapshot_retirement_metrics().peak, 1);

    let first = engine
        .snapshot_retirements
        .try_reserve()
        .expect("first retirement slot");
    let second = engine
        .snapshot_retirements
        .try_reserve()
        .expect("second retirement slot");
    engine.tick_tree_snapshot = Some(make_snapshot());
    engine.clear_tick_tree_snapshot();
    assert_eq!(engine.process_snapshot_retirement_metrics().rejected, 1);
    drop(first);
    drop(second);
}
