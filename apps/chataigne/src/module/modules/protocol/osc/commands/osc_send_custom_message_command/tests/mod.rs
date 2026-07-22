use golden_core::app::ProjectNode;
use golden_core::node::Node;
use golden_core::node::NodeMeta;

#[test]
fn osc_send_custom_message_command_decodes_from_project_node_type() {
    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        super::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
        &serde_json::Value::Null,
        &NodeMeta::new("Send Custom Message".to_string()),
    )
    .expect("OSC custom message command should decode from project files");

    assert_eq!(node.get_type(), super::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE);
}
