use std::{net::UdpSocket, time::Duration};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch, NodeUserPermissions},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, RangeConstraint},
    process_ctx::ExecutionPhase,
    script::{ScriptSource, ScriptUiSource},
    ui_sync::UiEditIntent,
};
use rosc::{decoder, OscPacket, OscType};

use crate::app::{
    AppNode, GenericOscModule, ModuleManager, OscDecodedMessage, OscSendCustomMessageCommand, OscValuePayload,
    OutputsManager,
};

#[test]
fn osc_module_declares_compiled_runtime_kernel() {
    assert_eq!(
        GenericOscModule::create().execution_rule().compiled_kernel_key,
        Some("chataigne.runtime.osc")
    );
}

#[test]
fn osc_module_script_descriptor_advertises_message_send_method() {
    let descriptor = GenericOscModule::create().engine_script_descriptor();

    assert!(
        descriptor.methods.iter().any(|candidate| candidate == "sendMessage"),
        "osc script descriptor should advertise 'sendMessage'"
    );
}

#[test]
fn osc_module_script_template_scaffolds_osc_callbacks_only() {
    let osc_module_type = <GenericOscModule as golden_core::node::DeclaredUserItemNode>::ITEM_NODE_TYPE;
    let config = crate::app::module::script_api::module_script_config(osc_module_type);
    let ScriptSource::Inline(source) = config.source else {
        panic!("osc module script template should resolve to inline source");
    };

    assert!(
        source.contains("function messageReceived"),
        "osc module template should include OSC callbacks; node_type={}, source={source}",
        osc_module_type,
    );
    assert!(source.contains("// Golden Core script template."));
    assert!(source.contains("function init()"));
    assert!(!source.contains("function noteOnReceived"));
    assert!(!source.contains("function clientConnected"));
}

#[test]
fn creating_script_under_osc_module_scaffolds_osc_callbacks() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");
    let module = engine.nodes.get(module_id).expect("module should exist");
    let module_type = module.get_type().to_string();
    let module_decl_id = module.node_data().meta.decl_id.0.clone();

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: module_id,
        node_type: "script".to_string(),
        label: Some("Script".to_string()),
        initial_params: Vec::new(),
    });
    assert!(create_ack.success, "creating a script under an OSC module should succeed");

    let script_id = find_child_by_key(&engine, module_id, "Script").unwrap_or_else(|| {
        let children = child_labels(&engine, module_id);
        panic!("created OSC module should expose a Script child; children were {children:?}");
    });
    let script_state = engine
        .ui_script_state(script_id)
        .expect("created script should expose UI script state");
    let ScriptUiSource::Inline { text } = script_state.config.source else {
        panic!("created OSC module script should use inline source");
    };

    assert!(
        text.contains("function messageReceived"),
        "created OSC module script should scaffold OSC callbacks; module_type={module_type}, module_decl_id={module_decl_id}, source={text}"
    );
    assert!(text.contains("// Golden Core script template."));
    assert!(text.contains("function init()"));
    assert!(
        !text.contains("function noteOnReceived"),
        "created OSC module script should not scaffold MIDI callbacks; module_type={module_type}, module_decl_id={module_decl_id}, source={text}"
    );
    assert!(
        !text.contains("function clientConnected"),
        "created OSC module script should not scaffold server stream callbacks; module_type={module_type}, module_decl_id={module_decl_id}, source={text}"
    );
}

#[test]
fn incoming_message_auto_adds_value_nodes_under_values_folder() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get_mut(module_id).expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.disable_transport_for_test();
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/foo".to_string(),
        payload: OscValuePayload::Single(ParamValue::Int(42)),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should process queued incoming OSC message");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(
        module.auto_add_enabled_for_test(),
        "auto-add should remain enabled by default"
    );
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "incoming message queue should be drained after runtime tick"
    );

    let values_folder = find_path(&engine, module_id, "values").expect("module should contain a Values folder");
    let value_child_labels = child_labels(&engine, values_folder);
    let values_param = find_child_by_key(&engine, values_folder, "foo").unwrap_or_else(|| {
        panic!("incoming OSC address should auto-create a parameter under Values; children were {value_child_labels:?}")
    });
    let values_node = engine
        .nodes
        .get(values_param)
        .expect("auto-created value node should exist");

    assert_eq!(values_node.node_data().meta.label, "foo");
    assert_eq!(
        values_node.node_data().meta.user_permissions,
        NodeUserPermissions::all()
    );
    assert_eq!(
        values_node.engine_param_snapshot().map(|snapshot| snapshot.value),
        Some(ParamValue::Int(42))
    );
}

#[test]
fn incoming_multi_message_auto_adds_missing_path_with_batched_trees() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get_mut(module_id).expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.disable_transport_for_test();
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/rig/arm/pose".to_string(),
        payload: OscValuePayload::Multi(vec![
            ParamValue::Float(1.0),
            ParamValue::Float(2.0),
            ParamValue::Float(3.0),
        ]),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should materialize missing parent path and multi-value leaf subtree");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "batched missing path and multi-value leaf creation should drain the incoming queue"
    );

    let first_value = find_path(&engine, module_id, "values/rig/arm/pose/value 1")
        .expect("first multi-value parameter should exist");
    let second_value = find_path(&engine, module_id, "values/rig/arm/pose/value 2")
        .expect("second multi-value parameter should exist");
    let third_value = find_path(&engine, module_id, "values/rig/arm/pose/value 3")
        .expect("third multi-value parameter should exist");

    assert_eq!(param_value(&engine, first_value), ParamValue::Float(1.0));
    assert_eq!(param_value(&engine, second_value), ParamValue::Float(2.0));
    assert_eq!(param_value(&engine, third_value), ParamValue::Float(3.0));
}

#[test]
fn incoming_messages_auto_add_shared_missing_path_in_one_tick() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    engine.resolve().expect("runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get_mut(module_id).expect("module should exist")
    else {
        panic!("expected GenericOscModule node");
    };
    module.disable_transport_for_test();
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/rig/arm/x".to_string(),
        payload: OscValuePayload::Single(ParamValue::Float(1.25)),
    });
    module.enqueue_incoming_message_for_test(OscDecodedMessage {
        address: "/rig/arm/y".to_string(),
        payload: OscValuePayload::Single(ParamValue::Float(2.5)),
    });

    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should materialize queued shared OSC path values");

    let crate::app::AppNode::GenericOscModule(module) = engine.nodes.get(module_id).expect("module should still exist")
    else {
        panic!("expected GenericOscModule node");
    };
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "batched shared missing path creation should drain the incoming queue"
    );

    let x_value = find_path(&engine, module_id, "values/rig/arm/x").expect("x parameter should exist");
    let y_value = find_path(&engine, module_id, "values/rig/arm/y").expect("y parameter should exist");

    assert_eq!(param_value(&engine, x_value), ParamValue::Float(1.25));
    assert_eq!(param_value(&engine, y_value), ParamValue::Float(2.5));
}

#[test]
fn new_module_command_tester_starts_empty() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    assert!(
        engine
            .nodes
            .get(command_tester_id)
            .and_then(|node| node.node_data().first_child)
            .is_none(),
        "new OSC modules should not seed command tester commands"
    );
}

#[test]
fn sparse_project_serialization_omits_unchanged_osc_defaults() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let round_tripped = golden_core::app::to_sparse_project_json_pretty(
        &golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode"),
    )
    .expect("round-tripped sparse project should encode");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&round_tripped).expect("round-tripped project json should parse"),
        serde_json::from_str::<serde_json::Value>(&json).expect("project json should parse"),
        "saving, reopening, then saving again should not add declared default noise"
    );

    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
    let module_record = value
        .get("root")
        .and_then(|root| root.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .and_then(|manager| manager.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .expect("osc module record should be saved under the module manager");

    assert_eq!(
        module_record.get("type"),
        Some(&serde_json::Value::String("osc_module".to_string()))
    );
    assert!(
        module_record.get("children").is_none(),
        "unchanged OSC connection, parameter, output, command tester, and values defaults should be recreated, not saved"
    );

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode");
    let loaded_manager = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should reload");
    let loaded_module = loaded
        .nodes
        .get(loaded_manager)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should reload");

    assert!(
        find_path(&loaded, loaded_module, "connection/connected").is_some(),
        "omitted connection defaults should be restored"
    );
    let loaded_outputs = find_path(&loaded, loaded_module, "connection/outputs")
        .expect("omitted output manager default should be restored");
    let loaded_default_output = loaded
        .nodes
        .get(loaded_outputs)
        .and_then(|outputs| outputs.node_data().first_child)
        .expect("omitted default OSC output should be restored");
    assert_eq!(
        loaded
            .nodes
            .get(loaded_default_output)
            .expect("restored default OSC output should exist")
            .get_type(),
        "osc_output"
    );
    let loaded_command_tester = find_path(&loaded, loaded_module, "command_tester")
        .expect("omitted command tester default should be restored");
    let loaded_command_types = loaded
        .nodes
        .get(loaded_command_tester)
        .expect("restored command tester should exist")
        .user_creatable_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();
    assert_eq!(
        loaded_command_types,
        vec![crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string()],
        "restored OSC command tester should retain its module-owned command catalog"
    );
    assert!(
        find_path(&loaded, loaded_module, "values").is_some(),
        "omitted values folder should be restored"
    );
}

#[test]
fn sparse_project_serialization_saves_only_changed_declared_osc_port_delta() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should be attached under manager");
    let port_id = find_path(&engine, module_id, "connection/input/port").expect("receiver port param should exist");
    set_param(&mut engine, port_id, ParamValue::Int(9001));
    engine.apply_edits().expect("receiver port edit should apply");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let round_tripped = golden_core::app::to_sparse_project_json_pretty(
        &golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode"),
    )
    .expect("round-tripped sparse project should encode");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&round_tripped).expect("round-tripped project json should parse"),
        serde_json::from_str::<serde_json::Value>(&json).expect("project json should parse"),
        "saving, reopening, then saving again should not add declared default noise"
    );

    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");
    let module_record = value
        .get("root")
        .and_then(|root| root.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .and_then(|manager| manager.get("children"))
        .and_then(serde_json::Value::as_array)
        .and_then(|children| children.first())
        .expect("osc module record should be saved under the module manager");
    let connection_record = json_child_by_decl(module_record, "connection");
    let input_record = json_child_by_decl(connection_record, "connection/input");
    let port_record = json_child_by_decl(input_record, "connection/input/port");

    assert_eq!(
        connection_record.get("meta"),
        Some(&serde_json::json!({ "decl_id": "connection" })),
        "declared ancestor folders should only carry their declaration path"
    );
    assert_eq!(
        input_record.get("meta"),
        Some(&serde_json::json!({ "decl_id": "connection/input" })),
        "declared receiver folder defaults should stay app-owned"
    );
    assert_eq!(
        port_record.get("meta"),
        Some(&serde_json::json!({ "decl_id": "connection/input/port" })),
        "declared port label and static metadata should stay app-owned"
    );
    assert_eq!(
        port_record.get("data"),
        Some(&serde_json::json!({ "value": { "Int": 9001 } })),
        "only the changed receiver port value should be persisted"
    );

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).expect("sparse project should decode");
    let loaded_manager = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should reload");
    let loaded_module = loaded
        .nodes
        .get(loaded_manager)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should reload");
    let loaded_port =
        find_path(&loaded, loaded_module, "connection/input/port").expect("receiver port should reload");
    let loaded_port_node = loaded.nodes.get(loaded_port).expect("receiver port node should exist");
    let snapshot = loaded_port_node
        .engine_param_snapshot()
        .expect("receiver port should remain a parameter");

    assert_eq!(loaded_port_node.node_data().meta.label, "Port");
    assert_eq!(snapshot.value, ParamValue::Int(9001));
    assert_eq!(snapshot.default_value, ParamValue::Int(9000));
    assert_eq!(snapshot.ui_hints.widget.as_deref(), Some("text"));
    assert!(matches!(
        snapshot.constraints.range,
        Some(RangeConstraint::Uniform {
            min: Some(0.0),
            max: Some(65535.0),
        })
    ));
}

#[test]
fn sparse_project_round_trip_preserves_saved_osc_command_tester_children() {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(ModuleManager::new().into(), None);
    engine.apply_edits().expect("module manager should attach");

    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should be attached under root");
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should be attached under manager");
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);
    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/save/reopen".to_string()),
    );
    engine.apply_edits().expect("command edit should apply");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json)
        .expect("sparse project with saved osc commands should decode");

    let loaded_manager = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module manager should reload");
    let loaded_module = loaded
        .nodes
        .get(loaded_manager)
        .and_then(|manager| manager.node_data().first_child)
        .expect("osc module should reload");
    let loaded_command_tester =
        find_path(&loaded, loaded_module, "command_tester").expect("command tester should reload");
    let loaded_command = loaded
        .nodes
        .get(loaded_command_tester)
        .and_then(|node| node.node_data().first_child)
        .expect("saved osc command should reload under the command tester");
    let loaded_address = find_path(&loaded, loaded_command, "address").expect("saved command address should reload");

    assert_eq!(
        loaded.nodes.get(loaded_command).map(|node| node.get_type()),
        Some(crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE),
        "saved osc command should keep its node type through project round-trip"
    );
    assert_eq!(
        loaded
            .nodes
            .get(loaded_address)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Str("/save/reopen".to_string())),
        "saved osc command parameters should survive project round-trip"
    );
}

#[test]
fn osc_module_connection_indicators_follow_receiver_and_outputs() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    let output_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();
    let (mut engine, module_id) = create_osc_module_with_output(output_port);

    let connected_param =
        find_path(&engine, module_id, "connection/connected").expect("module connection state parameter should exist");
    let can_receive_param = find_path(&engine, module_id, "connection/can_receive")
        .expect("module incoming capability parameter should exist");
    let can_send_param = find_path(&engine, module_id, "connection/can_send")
        .expect("module outgoing capability parameter should exist");
    let receiver_folder = find_path(&engine, module_id, "connection/input").expect("receiver folder should exist");
    let receiver_port_param =
        find_path(&engine, module_id, "connection/input/port").expect("receiver port param should exist");
    let outputs_id = find_path(&engine, module_id, "connection/outputs").expect("outputs folder should exist");

    assert!(
        engine
            .nodes
            .get(outputs_id)
            .expect("outputs node should exist")
            .node_data()
            .meta
            .can_be_disabled,
        "OSC Outputs should expose an enabled toggle"
    );

    assert_bool_param(
        &engine,
        connected_param,
        true,
        "output-only OSC transport should be connected",
    );
    assert_bool_param(
        &engine,
        can_receive_param,
        false,
        "disabled Receiver should hide the incoming indicator",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        true,
        "enabled Outputs should show the outgoing indicator",
    );

    set_node_enabled(&mut engine, outputs_id, false);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        false,
        "OSC module with Receiver and Outputs disabled should not report connected",
    );
    assert_bool_param(
        &engine,
        can_receive_param,
        false,
        "disabled Receiver should still hide the incoming indicator",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        false,
        "disabled Outputs should hide the outgoing indicator",
    );

    set_param(
        &mut engine,
        receiver_port_param,
        ParamValue::Int(i32::from(available_udp_port())),
    );
    set_node_enabled(&mut engine, receiver_folder, true);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        true,
        "OSC receiver should report connected after binding",
    );
    assert_bool_param(
        &engine,
        can_receive_param,
        true,
        "enabled Receiver should show the incoming indicator",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        false,
        "disabled Outputs should still hide the outgoing indicator",
    );

    set_node_enabled(&mut engine, outputs_id, true);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        can_receive_param,
        true,
        "enabled Receiver should keep the incoming indicator visible",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        true,
        "enabled Outputs should show the outgoing indicator again",
    );
}

#[test]
fn osc_module_root_enable_toggle_stops_and_restarts_transport() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    let output_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();
    let (mut engine, module_id) = create_osc_module_with_output(output_port);

    let connected_param =
        find_path(&engine, module_id, "connection/connected").expect("module connection state parameter should exist");
    assert_bool_param(
        &engine,
        connected_param,
        true,
        "OSC module should report connected while enabled",
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        false,
        "OSC module should disconnect as soon as its root node is disabled",
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        true,
        "OSC module should reconnect after its root node is re-enabled",
    );
}

#[test]
fn send_custom_message_command_sends_osc_packet_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);

    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    assert!(
        engine
            .nodes
            .get(command_tester_id)
            .and_then(|node| node.node_data().first_child)
            .is_none(),
        "command tester should start empty before commands are user-created"
    );

    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);

    let command_child_labels = child_labels(&engine, command_id);
    assert!(
        !command_child_labels.iter().any(|label| label == "Command"),
        "command children should be flat; children were {command_child_labels:?}"
    );

    let arguments_id = find_path(&engine, command_id, "arguments")
        .unwrap_or_else(|| panic!("arguments folder should exist; command children were {command_child_labels:?}"));
    engine.add_user_item(
        Parameter::new("Int", ParamValue::Int(7), ParameterChangeCheck::ValueChange).into(),
        Some(arguments_id),
    );
    engine.add_user_item(
        Parameter::new("Vec2", ParamValue::Vec2(1.5, 2.5), ParameterChangeCheck::ValueChange).into(),
        Some(arguments_id),
    );
    engine.add_user_item(
        Parameter::new(
            "String",
            ParamValue::Str("hello".to_string()),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(arguments_id),
    );

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    let trigger_param = find_path(&engine, command_id, "trigger").expect("command trigger param should exist");
    let auto_trigger_param =
        find_path(&engine, command_id, "auto_trigger").expect("command auto-trigger param should exist");
    assert!(
        !engine
            .nodes
            .get(trigger_param)
            .expect("trigger param should exist")
            .node_data()
            .meta
            .presentation
            .show_in_inspector_content,
        "trigger should be hidden from inspector content so the command inspector can render it in the header"
    );
    assert_eq!(
        engine
            .nodes
            .get(auto_trigger_param)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value),
        Some(ParamValue::Bool(false)),
        "command tester auto-trigger should be available and disabled by default"
    );
    assert!(
        !engine
            .nodes
            .get(auto_trigger_param)
            .expect("auto-trigger param should exist")
            .node_data()
            .meta
            .presentation
            .show_in_inspector_content,
        "auto-trigger should be hidden from inspector content so the command inspector can render it in the header"
    );
    assert!(
        find_path(&engine, command_id, "last_result").is_none(),
        "commands should not expose a last result parameter"
    );

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/custom-message".to_string()),
    );
    engine.apply_edits().expect("command setup edits should apply");

    engine.edits.push(Edit::SetParam {
        node: trigger_param,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("command trigger edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("command trigger should dispatch");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued command request should dispatch to the module");
    engine
        .apply_edits()
        .expect("queued command request side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("custom OSC command should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/custom-message");
            assert_eq!(
                message.args,
                vec![
                    OscType::Int(7),
                    OscType::Float(1.5),
                    OscType::Float(2.5),
                    OscType::String("hello".to_string()),
                ]
            );
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn execute_event_runs_osc_command_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/execute-event".to_string()),
    );
    engine.apply_edits().expect("command setup edits should apply");

    // Fire the command the way the state-machine output dispatch does: a single
    // execute event targeting the command (not a trigger-param edit), so the path
    // works per-lane under multiplex.
    let execute_event = golden_core::events::CustomEvent::new(
        crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
        Some(command_id),
        serde_json::to_value(crate::app::module_command::ModuleCommandExecuteEvent {
            command_id,
            param_overrides: Vec::new(),
        })
        .expect("execute event payload should serialize"),
    );
    engine.edits.push(Edit::EmitCustomEvent { event: execute_event });
    engine.apply_edits().expect("execute event edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("execute event should dispatch to the command");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued command request should dispatch to the module");
    engine
        .apply_edits()
        .expect("queued command request side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("execute event should cause the command to send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/execute-event");
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn output_manager_creates_module_linked_osc_command() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);
    engine.add_node(OutputsManager::new().into(), None);
    engine.apply_edits().expect("outputs manager should attach");

    let output_manager_id = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == OutputsManager::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("outputs manager should exist");
    let module_label = engine
        .nodes
        .get(module_id)
        .expect("OSC module should exist")
        .node_data()
        .meta
        .label
        .clone();
    let catalog = engine.catalog_creatable_items(output_manager_id);
    assert!(
        catalog.iter().any(|item| {
            item.node_type
                == crate::app::state_machine_nodes_generic_commands::GenericLogCommand::NODE_TYPE
                && item.menu_path == vec!["Generic".to_string()]
        }),
        "generic output commands should be grouped under Generic; catalog was {catalog:?}"
    );
    let command_item = catalog
        .into_iter()
        .find(|item| {
            item.node_type == crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE
                && item.menu_path.first() == Some(&module_label)
        })
        .expect("outputs manager should expose an OSC command under the OSC module");
    assert_eq!(
        command_item.item_kind,
        crate::app::module_command::MODULE_COMMAND_ITEM_KIND
    );

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: output_manager_id,
        node_type: command_item.node_type.clone(),
        label: Some(command_item.label.clone()),
        initial_params: command_item
            .initial_params
            .into_iter()
            .map(Into::into)
            .collect(),
    });
    assert!(
        create_ack.success,
        "module-linked output command should be creatable: {create_ack:?}"
    );

    let command_id = find_direct_child_by_type(
        &engine,
        output_manager_id,
        crate::app::OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE,
    )
    .expect("outputs manager should contain the created OSC command");
    let target_module = find_path(
        &engine,
        command_id,
        crate::app::module_command::MODULE_COMMAND_TARGET_MODULE_PATH,
    )
    .expect("output-created command should include a target module reference");
    let module_uuid = engine
        .nodes
        .get(module_id)
        .expect("OSC module should exist")
        .node_data()
        .meta
        .uuid;
    assert_eq!(
        engine
            .nodes
            .get(target_module)
            .and_then(|node| node.engine_param_snapshot())
            .and_then(|snapshot| match snapshot.value {
                ParamValue::Reference(reference) => Some(reference.uuid()),
                _ => None,
            }),
        Some(module_uuid)
    );

    let address_param =
        find_path(&engine, command_id, "address").expect("command address param should exist");
    let trigger_param =
        find_path(&engine, command_id, "trigger").expect("command trigger param should exist");
    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/output-manager".to_string()),
    );
    engine
        .apply_edits()
        .expect("command setup edits should apply");

    engine.edits.push(Edit::SetParam {
        node: trigger_param,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("command trigger edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("command trigger should dispatch");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued command request should dispatch to the linked module");
    engine
        .apply_edits()
        .expect("queued command side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("linked OSC command should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/output-manager");
            assert!(message.args.is_empty());
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn auto_trigger_send_custom_message_command_sends_when_command_parameter_changes() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);

    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = create_send_custom_message_command(&mut engine, command_tester_id);

    let address_param = find_path(&engine, command_id, "address").expect("command address param should exist");
    let auto_trigger_param =
        find_path(&engine, command_id, "auto_trigger").expect("command auto-trigger param should exist");

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/before-auto-trigger".to_string()),
    );
    engine.apply_edits().expect("initial command setup edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("initial command setup event should dispatch");
    engine
        .apply_edits()
        .expect("initial command setup event should settle without sending");

    set_param(&mut engine, auto_trigger_param, ParamValue::Bool(true));
    engine.apply_edits().expect("auto-trigger edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("auto-trigger edit should dispatch");
    engine
        .apply_edits()
        .expect("auto-trigger edit should not emit a command request");

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/auto-trigger".to_string()),
    );
    engine
        .apply_edits()
        .expect("auto-triggered command parameter edit should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("auto-triggered command parameter edit should dispatch");
    engine
        .apply_edits()
        .expect("queued auto-triggered command request should apply through the engine");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued auto-triggered command request should dispatch to the module");
    engine
        .apply_edits()
        .expect("queued auto-triggered command request side effects should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the auto-triggered command");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("auto-triggered OSC command should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/test/auto-trigger");
            assert!(message.args.is_empty());
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

#[test]
fn changing_values_parameter_sends_osc_packet_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

    let (mut engine, module_id) = create_osc_module_with_output(receiver_port);

    let values_id = find_path(&engine, module_id, "values").expect("values folder should exist");
    
    // Enable auto_feedback to allow sending values
    let auto_feedback_id = find_path(&engine, module_id, "parameters/processing/auto_feedback")
        .expect("auto_feedback parameter should exist");
    set_param(&mut engine, auto_feedback_id, ParamValue::Bool(true));
    engine.apply_edits().expect("auto_feedback change should apply");

    engine.add_node(
        Parameter::new("Foo", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(values_id),
    );
    engine.apply_edits().expect("value node should be created");

    let value_id = find_child_by_key(&engine, values_id, "Foo").expect("value parameter should exist");
    set_param(&mut engine, value_id, ParamValue::Float(3.25));
    engine.apply_edits().expect("value change should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("value change should dispatch");
    engine.apply_edits().expect("outbound value edit queue should settle");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should flush outbound OSC values");

    let mut buffer = [0u8; 2048];
    let (length, _) = receiver
        .recv_from(&mut buffer)
        .expect("changing a values parameter should send a UDP packet");
    let (_, packet) = decoder::decode_udp(&buffer[..length]).expect("udp payload should decode as osc");

    match packet {
        OscPacket::Message(message) => {
            assert_eq!(message.addr, "/Foo");
            assert_eq!(message.args, vec![OscType::Float(3.25)]);
        }
        other => panic!("expected OSC message packet, got {other:?}"),
    }
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        let meta = &node.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }

    None
}

fn find_direct_child_by_type(
    engine: &crate::app::AppEngine,
    parent: NodeId,
    node_type: &str,
) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id)?;
        if node.get_type() == node_type {
            return Some(child_id);
        }
        child = node.node_data().next_sibling;
    }

    None
}

fn child_labels(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut labels = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id).expect("child node should exist");
        labels.push(node.node_data().meta.label.clone());
        child = node.node_data().next_sibling;
    }
    labels
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    let mut remaining = path.trim_matches('/');

    loop {
        if remaining.is_empty() {
            return Some(current);
        }

        if let Some(found) = find_child_by_key(engine, current, remaining) {
            return Some(found);
        }

        let Some((segment, tail)) = remaining.split_once('/') else {
            return find_child_by_key(engine, current, remaining);
        };
        current = find_child_by_key(engine, current, segment)?;
        remaining = tail;
    }
}

fn json_child_by_decl<'a>(record: &'a serde_json::Value, decl_id: &str) -> &'a serde_json::Value {
    record
        .get("children")
        .and_then(serde_json::Value::as_array)
        .and_then(|children| {
            children.iter().find(|child| {
                child
                    .get("meta")
                    .and_then(|meta| meta.get("decl_id"))
                    .and_then(serde_json::Value::as_str)
                    == Some(decl_id)
            })
        })
        .unwrap_or_else(|| panic!("child record '{decl_id}' should exist"))
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn set_node_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
}

fn param_value(engine: &crate::app::AppEngine, node: NodeId) -> ParamValue {
    engine
        .nodes
        .get(node)
        .and_then(|candidate| candidate.engine_param_snapshot())
        .map(|snapshot| snapshot.value)
        .unwrap_or_else(|| panic!("expected parameter value for node {node:?}"))
}

fn settle_osc_module_state(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().expect("pending OSC indicator edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending OSC indicator edits should dispatch");
    engine
        .apply_edits()
        .expect("OSC indicator event reactions should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should refresh OSC indicator state");
    engine
        .apply_edits()
        .expect("OSC indicator parameter updates should apply");
}

fn assert_bool_param(engine: &crate::app::AppEngine, node: NodeId, expected: bool, context: &str) {
    assert_eq!(bool_param_value(engine, node), expected, "{context}");
}

fn bool_param_value(engine: &crate::app::AppEngine, node: NodeId) -> bool {
    match engine
        .nodes
        .get(node)
        .and_then(|candidate| candidate.engine_param_snapshot())
        .map(|snapshot| snapshot.value)
    {
        Some(ParamValue::Bool(value)) => value,
        other => panic!("expected bool parameter, got {other:?}"),
    }
}

fn available_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .expect("temporary UDP port probe should bind")
        .local_addr()
        .expect("temporary UDP port probe should expose an address")
        .port()
}

fn create_send_custom_message_command(engine: &mut crate::app::AppEngine, command_tester_id: NodeId) -> NodeId {
    engine.add_user_item(OscSendCustomMessageCommand::create().into(), Some(command_tester_id));
    engine
        .apply_edits()
        .expect("send custom message command should be created");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("command tester should decorate the created command");
    engine
        .apply_edits()
        .expect("command tester controls should materialize");
    for _ in 0..4 {
        engine
            .apply_edits()
            .expect("send custom message command children should materialize");
    }

    engine
        .nodes
        .get(command_tester_id)
        .and_then(|node| node.node_data().first_child)
        .expect("send custom message command should be created")
}

fn create_osc_module_with_output(receiver_port: u16) -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let receiver_folder = find_path(&engine, module_id, "connection/input").expect("receiver folder should exist");
    engine.edits.push(Edit::PatchMeta {
        node: receiver_folder,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });

    let outputs_id = find_path(&engine, module_id, "connection/outputs").expect("outputs folder should exist");
    let output_id = engine
        .nodes
        .get(outputs_id)
        .and_then(|node| node.node_data().first_child)
        .expect("default output should be created");

    let remote_host_param = find_path(&engine, output_id, "remote_host").expect("output host param should exist");
    let remote_port_param = find_path(&engine, output_id, "remote_port").expect("output port param should exist");
    set_param(&mut engine, remote_host_param, ParamValue::Str("127.0.0.1".to_string()));
    set_param(
        &mut engine,
        remote_port_param,
        ParamValue::Int(i32::from(receiver_port)),
    );

    engine.apply_edits().expect("osc transport config edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("osc transport config edits should dispatch");
    engine
        .apply_edits()
        .expect("osc transport config reactions should apply");
    engine.resolve().expect("runtime schedule should resolve");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should refresh osc transport");

    (engine, module_id)
}
