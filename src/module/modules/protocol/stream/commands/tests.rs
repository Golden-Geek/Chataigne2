use golden_core::app::ProjectNode;
use golden_core::node::Node;
use golden_core::node::NodeMeta;

use super::{
    StreamingSendBytesCommand, StreamingSendHexStringCommand,
    StreamingSendStringCommand, StreamingSendValuesCommand, STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
    STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE, STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
    STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
};

#[test]
fn streaming_commands_are_module_command_items() {
    let commands: Vec<Box<dyn Node>> = vec![
        Box::new(StreamingSendStringCommand::create()),
        Box::new(StreamingSendBytesCommand::create()),
        Box::new(StreamingSendHexStringCommand::create()),
        Box::new(StreamingSendValuesCommand::create()),
    ];

    for command in commands {
        assert_eq!(
            command.user_item_kind(),
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            "streaming command '{}' should register as a module command item",
            command.get_type()
        );
    }
}

#[test]
fn command_tester_accepts_streaming_command_items() {
    let tester = crate::app::ModuleCommandTester::create(
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
    );
    let commands: Vec<Box<dyn Node>> = vec![
        Box::new(StreamingSendStringCommand::create()),
        Box::new(StreamingSendBytesCommand::create()),
        Box::new(StreamingSendHexStringCommand::create()),
        Box::new(StreamingSendValuesCommand::create()),
    ];

    for command in commands {
        assert!(
            tester.user_container_accepts_item(command.get_type(), command.user_item_kind()),
            "streaming command tester should accept '{}' as '{}'",
            command.get_type(),
            command.user_item_kind()
        );
    }
}

#[test]
fn streaming_command_nodes_decode_from_project_node_type() {
    let node_types = [
        "module_command_tester",
        STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
        STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
        STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE,
        STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
    ];

    for node_type in node_types {
        let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
            node_type,
            &serde_json::Value::Null,
            &NodeMeta::new("Decoded Node".to_string()),
        )
        .unwrap_or_else(|error| panic!("{node_type} should decode from project files: {error}"));

        assert_eq!(node.get_type(), node_type);
    }
}
