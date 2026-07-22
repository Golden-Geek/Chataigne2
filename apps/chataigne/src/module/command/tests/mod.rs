use golden_core::{
    app::ProjectNode,
    edit::Edit,
    engine::EngineTime,
    events::CustomEventRetention,
    node::{Node, NodeId, NodeMeta},
    process_ctx::{ExecutionPhase, ProcessCtx},
};

use super::{
    emit_command_execute_with_invocation, ModuleCommandDeliveryPolicy,
    ModuleCommandInvocationId, ModuleCommandTester, MODULE_COMMAND_ITEM_KIND,
};

#[test]
fn internal_command_execute_events_are_transient_and_keep_invocation_identity() {
    let command = NodeId(20);
    let invocation_id = ModuleCommandInvocationId::new(NodeId(10), 7);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 1,
            micro: 0,
            seq: 0,
        },
    );

    emit_command_execute_with_invocation(
        &mut ctx,
        command,
        Vec::new(),
        Some(invocation_id),
        ModuleCommandDeliveryPolicy::Standard,
    )
    .expect("command execute should serialize");

    let Edit::EmitCustomEvent { event } = &ctx
        .edits
        .pending
        .last()
        .expect("command execute should enqueue one custom event")
        .edit
    else {
        panic!("command execute should enqueue a custom event");
    };
    assert_eq!(event.retention, CustomEventRetention::Transient);
    let decoded = super::command_execute_request(event, command)
        .expect("transient event should retain its typed command payload");
    assert_eq!(decoded.invocation_id, Some(invocation_id));
}

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
fn module_command_tester_uses_owned_command_catalog() {
    let tester = ModuleCommandTester::create_owned(vec![
        crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string(),
    ]);

    let item_types = tester
        .user_creatable_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();
    assert_eq!(
        item_types,
        vec![crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string()]
    );
    assert!(!tester.user_container_accepts_item(
        crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
        MODULE_COMMAND_ITEM_KIND,
    ));
}

#[test]
fn empty_module_command_catalog_remains_empty() {
    let tester = ModuleCommandTester::create_owned(Vec::new());

    assert!(tester.user_creatable_items().is_empty());
    assert!(!tester.user_container_accepts_item(
        crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
        MODULE_COMMAND_ITEM_KIND,
    ));
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
fn decoded_module_command_tester_restores_persisted_command_catalog() {
    let tester = ModuleCommandTester::create(
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
    );
    let data = tester
        .project_encode_data()
        .expect("module command tester should encode its advertised catalog");

    let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
        "module_command_tester",
        &data,
        &NodeMeta::new("Command Tester".to_string()),
    )
    .expect("module command tester should decode from project files");

    let items = node.user_creatable_items();
    let item_types = items.iter().map(|item| item.node_type.as_str()).collect::<Vec<_>>();

    assert_eq!(
        item_types,
        crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES,
        "decoded testers should restore their persisted advertised command order"
    );
    assert!(!items.iter().any(|item| item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE));
    assert!(
        node.user_container_accepts_item(
            crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "decoded testers should accept persisted advertised command items"
    );
    assert!(
        !node.user_container_accepts_item(
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "decoded testers should reject commands outside their persisted advertised catalog"
    );
}

#[test]
fn decoded_legacy_module_command_tester_accepts_declared_module_commands_without_catalog_data() {
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
        "legacy decoded testers should accept saved OSC command items without persisted catalog data"
    );
    assert!(
        items.iter().any(|item| {
            item.node_type == crate::app::module::common::streaming::commands::STREAMING_SEND_STRING_COMMAND_NODE_TYPE
        }),
        "legacy decoded testers should accept saved streaming command items without persisted catalog data"
    );
    assert!(
        node.user_container_accepts_item(
            crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
            MODULE_COMMAND_ITEM_KIND,
        ),
        "legacy decoded testers should accept saved OSC command items without persisted catalog data"
    );
}
