use super::*;
use golden_core::{
    node::{DashboardWidgetTargetDescriptor, NodeUuid, PresentationHint},
    process_ctx::ProcessTreeNodeSnapshot,
};
use std::collections::HashMap;

fn node(id: u64, parent: Option<u64>, kind: &str) -> ProcessTreeNodeSnapshot {
    ProcessTreeNodeSnapshot {
        id: NodeId(id),
        uuid: NodeUuid(uuid::Uuid::from_u128(u128::from(id))),
        parent: parent.map(NodeId),
        first_child: None,
        next_sibling: None,
        node_type: kind.into(),
        decl_id: String::new(),
        short_name: String::new(),
        label: String::new(),
        tags: Vec::new(),
        presentation: PresentationHint::default(),
        enabled: true,
        can_be_disabled: true,
        child_count: 0,
        param_value: None,
        param_constraints: None,
        param_control: None,
        dashboard_widget_target: DashboardWidgetTargetDescriptor::inspector_only(),
        script_properties: HashMap::new(),
        script_methods: Vec::new(),
    }
}

fn nested_snapshot(depth: u64, host: &str) -> ProcessTreeSnapshot {
    let mut nodes = HashMap::new();
    for id in 1..=depth + 1 {
        let mut folder = node(id, (id > 1).then_some(id - 1), "folder");
        folder.first_child = Some(NodeId(id + 1));
        nodes.insert(NodeId(id), folder);
    }
    let output = depth + 2;
    let mut target = node(output, Some(output - 1), UDP_OUTPUT_NODE_TYPE);
    target.first_child = Some(NodeId(output + 1));
    nodes.insert(NodeId(output), target);
    let mut host_param = node(output + 1, Some(output), "parameter");
    host_param.decl_id = "remote_host".into();
    host_param.param_value = Some(ParamValue::Str(host.into()));
    host_param.next_sibling = Some(NodeId(output + 2));
    nodes.insert(NodeId(output + 1), host_param);
    let mut port = node(output + 2, Some(output), "parameter");
    port.decl_id = "remote_port".into();
    port.param_value = Some(ParamValue::Int(9000));
    nodes.insert(NodeId(output + 2), port);
    ProcessTreeSnapshot::new(NodeId(1), nodes)
}

#[test]
fn deep_folder_preparation_uses_no_recursive_stack_or_output_id_accumulation() {
    let snapshot = nested_snapshot(20_000, "127.0.0.1");
    let plan = prepare(&snapshot, NodeId(1)).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan.target(0).remote_host, "127.0.0.1");
}

#[test]
fn unchanged_snapshot_reuses_plan_storage_and_invalid_hosts_reject_before_admission() {
    let snapshot = nested_snapshot(2, "127.0.0.1");
    let mut cache = None;
    let first = cached_plan(&mut cache, &snapshot, NodeId(1)).unwrap();
    let second = cached_plan(&mut cache, &snapshot, NodeId(1)).unwrap();
    assert!(std::ptr::eq(first.target(0), second.target(0)));
    let invalid = nested_snapshot(2, &"x".repeat(1025));
    assert!(cached_plan(&mut cache, &invalid, NodeId(1)).is_err());
    // Failed preparation cannot poison a later retry or retain a stale cached plan.
    let recovered = cached_plan(&mut cache, &snapshot, NodeId(1)).unwrap();
    assert_eq!(recovered.target(0).remote_host, "127.0.0.1");
}
