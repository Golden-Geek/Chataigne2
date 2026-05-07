use golden_core::{
    app::ProjectNode,
    node::{Node, NodeMeta},
};

use super::{HttpRequestCommand, HttpUploadFileCommand};
use crate::app::module::common::http::{
    HTTP_COMMAND_NODE_TYPES, HTTP_REQUEST_COMMAND_NODE_TYPE, HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE,
};

#[test]
fn http_commands_are_module_command_items() {
    let commands: Vec<Box<dyn Node>> = vec![
        Box::new(HttpRequestCommand::create()),
        Box::new(HttpUploadFileCommand::create()),
    ];

    for command in commands {
        assert_eq!(
            command.user_item_kind(),
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            "HTTP command '{}' should register as a module command item",
            command.get_type()
        );
    }
}

#[test]
fn command_tester_accepts_http_command_items() {
    let tester = crate::app::ModuleCommandTester::create(HTTP_COMMAND_NODE_TYPES);
    let commands: Vec<Box<dyn Node>> = vec![
        Box::new(HttpRequestCommand::create()),
        Box::new(HttpUploadFileCommand::create()),
    ];

    for command in commands {
        assert!(
            tester.user_container_accepts_item(command.get_type(), command.user_item_kind()),
            "HTTP command tester should accept '{}' as '{}'",
            command.get_type(),
            command.user_item_kind()
        );
    }
}

#[test]
fn http_command_nodes_decode_from_project_node_type() {
    let node_types = [
        "module_command_tester",
        HTTP_REQUEST_COMMAND_NODE_TYPE,
        HTTP_UPLOAD_FILE_COMMAND_NODE_TYPE,
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
