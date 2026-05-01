use std::{net::UdpSocket, time::Duration};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch, NodeUserPermissions},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
};
use rosc::{decoder, OscPacket, OscType};

use crate::app::{GenericOscModule, OscDecodedMessage, OscValuePayload};

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
fn send_custom_message_command_sends_osc_packet_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

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
    engine.resolve().expect("runtime schedule should resolve");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should refresh osc transport");

    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");
    let command_id = engine
        .nodes
        .get(command_tester_id)
        .and_then(|node| node.node_data().first_child)
        .expect("default send custom message command should be created");

    let command_child_labels = child_labels(&engine, command_id);
    let arguments_id = find_path(&engine, command_id, "command/arguments")
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

    let address_param = find_path(&engine, command_id, "command/address").expect("command address param should exist");
    let execute_param = find_path(&engine, command_id, "command/execute").expect("command execute param should exist");
    let last_result_param =
        find_path(&engine, command_id, "command/last_result").expect("command result param should exist");

    set_param(
        &mut engine,
        address_param,
        ParamValue::Str("/test/custom-message".to_string()),
    );
    engine.apply_edits().expect("command setup edits should apply");

    engine.edits.push(Edit::SetParam {
        node: execute_param,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("command trigger edit should apply");
    engine
        .apply_edits()
        .expect("queued command request should apply through the engine");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should let the transport process the queued command");

    let last_result = engine
        .nodes
        .get(last_result_param)
        .and_then(|node| node.engine_param_snapshot())
        .map(|snapshot| snapshot.value);
    assert_eq!(
        last_result,
        Some(ParamValue::Str(
            "Queued OSC /test/custom-message for 1 output(s)".to_string()
        ))
    );

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
fn changing_values_parameter_sends_osc_packet_through_module_output() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver should accept a read timeout");
    let receiver_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();

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
    engine.resolve().expect("runtime schedule should resolve");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("runtime tick should refresh osc transport");

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
        if meta.decl_id.0 == key || meta.short_name == key || meta.label == key {
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
