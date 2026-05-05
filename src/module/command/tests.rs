use golden_core::{
    app::ProjectNode,
    node::{Node, NodeMeta},
};

use super::{ModuleCommandTester, MODULE_COMMAND_ITEM_KIND};

#[test]
fn module_command_tester_uses_advertised_command_catalog() {
    let tester = ModuleCommandTester::create(
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
    );
    let items = tester.user_creatable_items();
    let item_types = items.iter().map(|item| item.node_type.as_str()).collect::<Vec<_>>();

    assert_eq!(
        items.len(),
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES.len()
    );
    assert!(items.iter().all(|item| item.item_kind == MODULE_COMMAND_ITEM_KIND));
    assert!(items.iter().all(|item| !item.select_when_created));
    assert!(items.iter().all(|item| {
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES.contains(&item.node_type.as_str())
    }));
    assert_eq!(
        item_types,
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
        "module command tester should preserve the advertised command order"
    );
    assert!(!items.iter().any(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE));
    assert!(
        tester.user_container_accepts_item(
            crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        )
    );
    assert!(
        !tester.user_container_accepts_item(
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        )
    );

    let created = tester
        .create_user_item(crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE)
        .expect("advertised command should be creatable");
    assert_eq!(
        created.get_type(),
        crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
    );
    assert!(tester.create_user_item("folder").is_none());
}

#[test]
fn module_command_tester_decodes_from_project_node_type() {
    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        "module_command_tester",
        &serde_json::Value::Null,
        &NodeMeta::new("Command Tester".to_string()),
    )
    .expect("module command tester should decode from project files");

    assert_eq!(node.get_type(), "module_command_tester");
}

#[test]
fn decoded_module_command_tester_accepts_declared_module_commands_until_scoped() {
    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        "module_command_tester",
        &serde_json::Value::Null,
        &NodeMeta::new("Command Tester".to_string()),
    )
    .expect("module command tester should decode from project files");

    let items = node.user_creatable_items();
    assert!(
        items
            .iter()
            .any(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE),
        "decoded testers should accept saved OSC command items before module init scopes the catalog"
    );
    assert!(
        items.iter().any(|item| {
            item.node_type == crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE
        }),
        "decoded testers should accept saved streaming command items before module init scopes the catalog"
    );
    assert!(
        node.user_container_accepts_item(
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "decoded testers should accept saved OSC command items before module init scopes the catalog"
    );
}
