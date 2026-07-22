use std::time::Duration;

use golden_core::{
    node::{Folder, Node, NodeId},
    parameter::ParamValue,
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{mqtt_publish_request_from_script, transport::MqttReceivedPublish, MqttModule};
use crate::app::module::common::mqtt::{MqttQos, MQTT_PUBLISH_COMMAND_NODE_TYPE};

#[test]
fn mqtt_module_command_tester_advertises_publish_command() {
    let (engine, module_id) = create_mqtt_module();
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    let available_types = engine
        .catalog_creatable_items(command_tester_id)
        .into_iter()
        .map(|item| item.node_type)
        .collect::<Vec<_>>();

    assert_eq!(
        available_types,
        vec![MQTT_PUBLISH_COMMAND_NODE_TYPE.to_string()],
        "MQTT command tester should advertise only MQTT publish commands"
    );
}

#[test]
fn mqtt_module_script_descriptor_advertises_publish_methods() {
    let descriptor = MqttModule::create().engine_script_descriptor();

    for method in ["publish", "publishText", "publishJson"] {
        assert!(
            descriptor.methods.iter().any(|candidate| candidate == method),
            "MQTT script descriptor should advertise '{method}'"
        );
    }
}

#[test]
fn mqtt_module_script_template_scaffolds_mqtt_callbacks_only() {
    let config = crate::app::module::script_api::module_script_config(MqttModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("MQTT module script template should resolve to inline source");
    };

    assert!(source.contains("local.publish"));
    assert!(source.contains("function messageReceived"));
    assert!(!source.contains("function noteOnReceived"));
    assert!(!source.contains("function clientConnected"));
}

#[test]
fn incoming_text_publish_auto_adds_value_nodes_from_topic() {
    let (mut engine, module_id) = create_mqtt_module();

    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MqttModule node");
    };
    module.enqueue_incoming_message_for_test(MqttReceivedPublish {
        topic: "sensors/temperature".to_string(),
        payload: b"21.5".to_vec(),
        qos: MqttQos::AtMost,
        retain: false,
    });

    run_mqtt_runtime_ticks(&mut engine, 4);

    let value_id = find_path(&engine, module_id, "values/sensors/temperature")
        .expect("MQTT topic should create nested value nodes");
    assert_eq!(
        param_value(&engine, value_id),
        ParamValue::Float(21.5),
        "text payloads should be parsed into scalar values"
    );

    let crate::app::AppNode::MqttModule(module) = engine.nodes.get(module_id).expect("module should still exist") else {
        panic!("expected MqttModule node");
    };
    assert!(
        !module.has_pending_incoming_messages_for_test(),
        "incoming MQTT queue should drain after value application"
    );
}

#[test]
fn incoming_json_publish_expands_under_topic() {
    let (mut engine, module_id) = create_mqtt_module();

    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MqttModule node");
    };
    module.enqueue_incoming_message_for_test(MqttReceivedPublish {
        topic: "devices/kitchen".to_string(),
        payload: br#"{"temperature":21,"humidity":0.42,"rgb":[1.0,0.5,0.0,1.0]}"#.to_vec(),
        qos: MqttQos::AtLeast,
        retain: true,
    });

    run_mqtt_runtime_ticks(&mut engine, 6);

    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/devices/kitchen/temperature")
                .expect("temperature value should be created"),
        ),
        ParamValue::Int(21)
    );
    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/devices/kitchen/humidity")
                .expect("humidity value should be created"),
        ),
        ParamValue::Float(0.42)
    );
    assert_eq!(
        param_value(
            &engine,
            find_path(&engine, module_id, "values/devices/kitchen/rgb").expect("rgb value should be created"),
        ),
        ParamValue::Color(1.0, 0.5, 0.0, 1.0)
    );
}

#[test]
fn script_publish_json_request_encodes_payload_qos_and_retain() {
    let request = mqtt_publish_request_from_script(
        "publishJson",
        &[
            ParamValue::Str("app/state".to_string()),
            ParamValue::Bool(true),
            ParamValue::Int(1),
            ParamValue::Bool(true),
        ],
    )
    .expect("publishJson should be handled")
    .expect("publishJson request should decode");

    assert_eq!(request.topic, "app/state");
    assert_eq!(request.payload, b"true");
    assert_eq!(request.qos, MqttQos::AtLeast);
    assert!(request.retain);
}

fn create_mqtt_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(MqttModule::create().into(), None);
    engine.apply_edits().expect("MQTT module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("MQTT defaults should materialize");
    }
    engine.resolve().expect("MQTT runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("MQTT module should be attached under root");

    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(module_id).expect("module should exist") else {
        panic!("expected MqttModule node");
    };
    module.disable_transport_for_test();

    (engine, module_id)
}

fn run_mqtt_runtime_ticks(engine: &mut crate::app::AppEngine, count: usize) {
    for _ in 0..count {
        engine
            .dispatch_inbox(ExecutionPhase::EngineTick)
            .expect("pending MQTT events should dispatch");
        engine.apply_edits().expect("pending MQTT event reactions should apply");
        engine
            .run_tick(Duration::from_millis(20))
            .expect("MQTT runtime tick should succeed");
        engine.apply_edits().expect("pending MQTT edits should apply");
        engine.resolve().expect("MQTT runtime schedule should resolve");
    }
}

fn param_value(engine: &crate::app::AppEngine, node: NodeId) -> ParamValue {
    engine
        .nodes
        .get(node)
        .and_then(|node| node.engine_param_snapshot())
        .map(|snapshot| snapshot.value)
        .expect("parameter value should exist")
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
