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

use crate::app::{AppNode, GenericOscModule, ModuleManager, OscSendCustomMessageCommand, OutputsManager};

mod authoring;
mod commands;
mod connection;
mod input;
mod outbound;
mod passive;
mod persistence;

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

fn find_direct_child_by_type(engine: &crate::app::AppEngine, parent: NodeId, node_type: &str) -> Option<NodeId> {
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
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        service_osc_module_turn(engine);
        let ready = engine.nodes.iter().all(|(_, node)| match node {
            AppNode::GenericOscModule(module) => module.base.transport_settled_for_test(),
            _ => true,
        });
        if ready {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "OSC native lifecycle must settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn service_osc_module_turn(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().expect("pending OSC edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("OSC inbox should dispatch");
    engine.apply_edits().expect("OSC reactions should apply");
    engine.run_tick(Duration::from_millis(20)).expect("OSC tick should run");
    engine.apply_edits().expect("OSC publication edits should apply");
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
    settle_osc_module_state(&mut engine);

    (engine, module_id)
}
