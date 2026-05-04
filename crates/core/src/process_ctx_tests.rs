use super::*;

#[test]
fn child_decl_lookup_matches_full_decl_id_or_final_segment() {
    let root_id = NodeId(1);
    let receiver_id = NodeId(2);
    let legacy_name_id = NodeId(3);

    let mut nodes = HashMap::new();
    nodes.insert(
        root_id,
        snapshot_node(root_id, None, Some(receiver_id), None, "root", "root", "Root"),
    );
    nodes.insert(
        receiver_id,
        snapshot_node(
            receiver_id,
            Some(root_id),
            None,
            Some(legacy_name_id),
            "parameters/receiver",
            "input",
            "Input",
        ),
    );
    nodes.insert(
        legacy_name_id,
        snapshot_node(
            legacy_name_id,
            Some(root_id),
            None,
            None,
            "legacy",
            "receiver",
            "Receiver",
        ),
    );

    let snapshot = ProcessTreeSnapshot::new(root_id, nodes);

    assert_eq!(snapshot.find_child_by_decl_id(root_id, "receiver"), Some(receiver_id));
    assert_eq!(
        snapshot.find_child_by_decl_id(root_id, "parameters/receiver"),
        Some(receiver_id)
    );
    assert_eq!(snapshot.find_child(root_id, "receiver"), Some(legacy_name_id));
}

fn snapshot_node(
    id: NodeId,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
    decl_id: &str,
    short_name: &str,
    label: &str,
) -> ProcessTreeNodeSnapshot {
    ProcessTreeNodeSnapshot {
        id,
        uuid: NodeUuid::nil(),
        parent,
        first_child,
        next_sibling,
        node_type: "folder".to_string(),
        decl_id: decl_id.to_string(),
        short_name: short_name.to_string(),
        label: label.to_string(),
        tags: Vec::new(),
        enabled: true,
        can_be_disabled: true,
        child_count: usize::from(first_child.is_some()),
        param_value: None,
        param_constraints: None,
        dashboard_widget_target: DashboardWidgetTargetDescriptor::inspector_only(),
        script_properties: HashMap::new(),
        script_methods: Vec::new(),
    }
}
