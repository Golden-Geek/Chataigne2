use std::{net::UdpSocket, time::Duration};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch, NodeUserPermissions},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};
use rosc::{decoder, OscPacket, OscType};

use crate::app::{GenericOscModule, OscDecodedMessage, OscSendCustomMessageCommand, OscValuePayload};

#[test]
fn incoming_message_auto_adds_value_nodes_under_values_folder() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("generic osc module should attach");
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
fn new_module_command_tester_starts_empty() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("generic osc module should attach");
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
fn osc_module_connection_indicators_follow_receiver_and_outputs() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    let output_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();
    let (mut engine, module_id) = create_osc_module_with_output(output_port);

    let connected_param = find_path(&engine, module_id, "connection/connected")
        .expect("module connection state parameter should exist");
    let can_receive_param = find_path(&engine, module_id, "connection/can_receive")
        .expect("module incoming capability parameter should exist");
    let can_send_param = find_path(&engine, module_id, "connection/can_send")
        .expect("module outgoing capability parameter should exist");
    let receiver_folder = find_path(&engine, module_id, "parameters/receiver").expect("receiver folder should exist");
    let receiver_port_param =
        find_path(&engine, module_id, "parameters/receiver/port").expect("receiver port param should exist");
    let outputs_id = find_path(&engine, module_id, "parameters/outputs").expect("outputs folder should exist");

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

    assert_bool_param(&engine, connected_param, true, "output-only OSC transport should be connected");
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
    engine.apply_edits().expect("generic osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");

    let receiver_folder = find_path(&engine, module_id, "parameters/receiver").expect("receiver folder should exist");
    engine.edits.push(Edit::PatchMeta {
        node: receiver_folder,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });

    let outputs_id = find_path(&engine, module_id, "parameters/outputs").expect("outputs folder should exist");
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
