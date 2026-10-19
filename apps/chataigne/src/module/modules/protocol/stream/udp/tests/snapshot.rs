use super::super::UDP_OUTPUT_NODE_TYPE;
use golden_core::{
    node::{DashboardWidgetTargetDescriptor, NodeId, NodeUuid, PresentationHint},
    parameter::ParamValue,
    process_ctx::{ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
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

pub(crate) fn nested_snapshot(depth: u64, host: &str, remote_port: u16) -> ProcessTreeSnapshot {
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
    port.param_value = Some(ParamValue::Int(i32::from(remote_port)));
    nodes.insert(NodeId(output + 2), port);
    ProcessTreeSnapshot::new(NodeId(1), nodes)
}
