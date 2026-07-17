use golden_core::node::Node;

use crate::app::module_modules_system_node_control_commands::{
    NodeControlRequest, NODE_SET_VALUE_COMMAND_NODE_TYPE, NODE_TRIGGER_COMMAND_NODE_TYPE,
};
use crate::app::{NodeSetValueCommand, NodeTriggerCommand};

#[test]
fn node_commands_are_project_creatable() {
    assert!(NodeSetValueCommand::project_create(NODE_SET_VALUE_COMMAND_NODE_TYPE).is_some());
    assert!(NodeTriggerCommand::project_create(NODE_TRIGGER_COMMAND_NODE_TYPE).is_some());
}

#[test]
fn node_command_payloads_keep_typed_values() {
    let request = NodeControlRequest::SetValue {
        target: golden_core::node::NodeId(42),
        value: golden_core::parameter::ParamValue::Float(0.5),
    };
    let encoded = serde_json::to_value(&request).unwrap();
    assert_eq!(
        serde_json::from_value::<NodeControlRequest>(encoded).unwrap(),
        request
    );
}
