use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, Instant};

use golden_core::edit::Edit;
use golden_core::node::{Node, NodeId};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use golden_core::ui_sync::UiEditIntent;
use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};
use serde_json::{json, Value};

use crate::app::{AppEngine, AppNode, GenericOscModule, ModuleManager, OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE};

use super::digest;
use super::engine_helpers::{
    drive_effects, find_child_by_type, find_node_by_type, find_path, materialize, param_value, set_param,
};

const INPUT_ADDRESS: &str = "/evidence/input";
const FIRST_OUTPUT_ADDRESS: &str = "/evidence/output/1";
const SECOND_OUTPUT_ADDRESS: &str = "/evidence/output/2";

pub(super) fn run() -> Result<Value, String> {
    let output_receiver = UdpSocket::bind("127.0.0.1:0")
        .map_err(|error| format!("failed to bind OSC evidence output receiver: {error}"))?;
    output_receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("failed to configure OSC output timeout: {error}"))?;
    let output_port = output_receiver
        .local_addr()
        .map_err(|error| format!("failed to inspect OSC output receiver: {error}"))?
        .port();
    let input_port = available_udp_port()?;

    let mut engine = golden_core::app::create_new_project_engine::<AppNode>()
        .map_err(|error| format!("failed to create the real Chataigne app engine: {error}"))?;
    materialize(&mut engine, 8)?;
    let module_id = create_and_configure_module(&mut engine, input_port, output_port)?;
    let command_ack = create_command(&mut engine, module_id)?;

    send_input(input_port)?;
    let input_value = wait_for_input(&mut engine, module_id)?;

    trigger_command(&mut engine, module_id, FIRST_OUTPUT_ADDRESS)?;
    trigger_command(&mut engine, module_id, SECOND_OUTPUT_ADDRESS)?;
    let effect_order = receive_output_order(&output_receiver, 2)?;
    let expected_order = vec![FIRST_OUTPUT_ADDRESS.to_string(), SECOND_OUTPUT_ADDRESS.to_string()];
    if effect_order != expected_order {
        return Err(format!(
            "OSC command effects were observed out of order: expected {expected_order:?}, got {effect_order:?}"
        ));
    }

    let before_reload = capture_saved_semantics(&engine, module_id)?;
    let before_reload_digest = digest::semantic_digest(&before_reload)?;
    let project_json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .map_err(|error| format!("failed to save OSC evidence project: {error}"))?;
    drop(engine);

    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&project_json)
        .map_err(|error| format!("failed to reload OSC evidence project: {error}"))?;
    let loaded_module = find_node_by_type(&loaded, GenericOscModule::NODE_TYPE)
        .ok_or_else(|| "reloaded project did not contain the OSC module".to_string())?;
    let after_reload = capture_saved_semantics(&loaded, loaded_module)?;
    let after_reload_digest = digest::semantic_digest(&after_reload)?;
    if before_reload != after_reload || before_reload_digest != after_reload_digest {
        return Err(format!(
            "OSC semantic state changed after save/reload: before={before_reload}, after={after_reload}"
        ));
    }

    Ok(json!({
        "command_creation_ack": command_ack,
        "input": {
            "address": INPUT_ADDRESS,
            "value": input_value,
        },
        "effect_order": effect_order,
        "save_reload": {
            "semantic_digest": after_reload_digest,
            "state": after_reload,
        },
    }))
}

fn create_and_configure_module(engine: &mut AppEngine, input_port: u16, output_port: u16) -> Result<NodeId, String> {
    let manager_id = find_node_by_type(engine, ModuleManager::NODE_TYPE)
        .ok_or_else(|| "real Chataigne app engine did not contain a Module Manager".to_string())?;
    engine.add_user_item(GenericOscModule::create().into(), Some(manager_id));
    materialize(engine, 6)?;
    let module_id = find_child_by_type(engine, manager_id, GenericOscModule::NODE_TYPE)
        .ok_or_else(|| "OSC module did not attach below Module Manager".to_string())?;

    let receiver_port = find_path(engine, module_id, "connection/input/port")
        .ok_or_else(|| "OSC receiver port parameter did not materialize".to_string())?;
    let outputs = find_path(engine, module_id, "connection/outputs")
        .ok_or_else(|| "OSC Outputs folder did not materialize".to_string())?;
    let output = engine
        .nodes
        .get(outputs)
        .and_then(|node| node.node_data().first_child)
        .ok_or_else(|| "OSC default output did not materialize".to_string())?;
    let output_host = find_path(engine, output, "remote_host")
        .ok_or_else(|| "OSC output host parameter did not materialize".to_string())?;
    let output_port_param = find_path(engine, output, "remote_port")
        .ok_or_else(|| "OSC output port parameter did not materialize".to_string())?;

    set_param(engine, receiver_port, ParamValue::Int(i32::from(input_port)));
    set_param(engine, output_host, ParamValue::Str("127.0.0.1".to_string()));
    set_param(engine, output_port_param, ParamValue::Int(i32::from(output_port)));
    drive_effects(engine)?;
    engine.resolve().map_err(|error| error.to_string())?;
    Ok(module_id)
}

fn create_command(engine: &mut AppEngine, module_id: NodeId) -> Result<bool, String> {
    let command_tester = find_path(engine, module_id, "command_tester")
        .ok_or_else(|| "OSC command tester did not materialize".to_string())?;
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: command_tester,
        node_type: OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE.to_string(),
        label: Some("Evidence Send".to_string()),
        initial_params: Vec::new(),
    });
    if !ack.success {
        return Err(format!("OSC command creation intent failed: {ack:?}"));
    }
    materialize(engine, 6)?;
    find_child_by_type(engine, command_tester, OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE)
        .ok_or_else(|| "OSC command did not materialize after successful UI acknowledgement".to_string())?;
    Ok(ack.success)
}

fn send_input(input_port: u16) -> Result<(), String> {
    let packet = OscPacket::Message(OscMessage {
        addr: INPUT_ADDRESS.to_string(),
        args: vec![OscType::Int(42)],
    });
    let bytes = encoder::encode(&packet).map_err(|error| format!("failed to encode OSC evidence input: {error}"))?;
    let sender =
        UdpSocket::bind("127.0.0.1:0").map_err(|error| format!("failed to bind OSC evidence input sender: {error}"))?;
    sender
        .send_to(&bytes, ("127.0.0.1", input_port))
        .map_err(|error| format!("failed to send OSC evidence input: {error}"))?;
    Ok(())
}

fn wait_for_input(engine: &mut AppEngine, module_id: NodeId) -> Result<i32, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        drive_effects(engine)?;
        if let Some(input) = find_path(engine, module_id, "values/evidence/input") {
            if let Some(ParamValue::Int(value)) = param_value(engine, input) {
                return Ok(value);
            }
        }
        if Instant::now() >= deadline {
            return Err("OSC input was not observable in the real module Values tree within 2 seconds".to_string());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn trigger_command(engine: &mut AppEngine, module_id: NodeId, address: &str) -> Result<(), String> {
    let command_tester =
        find_path(engine, module_id, "command_tester").ok_or_else(|| "OSC command tester disappeared".to_string())?;
    let command = find_child_by_type(engine, command_tester, OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE)
        .ok_or_else(|| "OSC command disappeared".to_string())?;
    let address_param = find_path(engine, command, "address")
        .ok_or_else(|| "OSC command address parameter did not materialize".to_string())?;
    let trigger_param = find_path(engine, command, "trigger")
        .ok_or_else(|| "OSC command trigger parameter did not materialize".to_string())?;

    set_param(engine, address_param, ParamValue::Str(address.to_string()));
    engine.apply_edits().map_err(|error| error.to_string())?;
    engine.edits.push(Edit::SetParam {
        node: trigger_param,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    drive_effects(engine)
}

fn receive_output_order(receiver: &UdpSocket, count: usize) -> Result<Vec<String>, String> {
    let mut order = Vec::with_capacity(count);
    let mut buffer = [0_u8; 2048];
    for _ in 0..count {
        let (length, _) = receiver
            .recv_from(&mut buffer)
            .map_err(|error| format!("failed to observe OSC command effect: {error}"))?;
        let (_, packet) = decoder::decode_udp(&buffer[..length])
            .map_err(|error| format!("failed to decode OSC command effect: {error}"))?;
        let OscPacket::Message(message) = packet else {
            return Err(format!("expected an OSC message effect, got {packet:?}"));
        };
        order.push(message.addr);
    }
    Ok(order)
}

fn capture_saved_semantics(engine: &AppEngine, module_id: NodeId) -> Result<Value, String> {
    let input = find_path(engine, module_id, "values/evidence/input")
        .ok_or_else(|| "saved OSC input value was missing".to_string())?;
    let command_tester = find_path(engine, module_id, "command_tester")
        .ok_or_else(|| "saved OSC command tester was missing".to_string())?;
    let command = find_child_by_type(engine, command_tester, OSC_SEND_CUSTOM_MESSAGE_COMMAND_NODE_TYPE)
        .ok_or_else(|| "saved OSC command was missing".to_string())?;
    let address =
        find_path(engine, command, "address").ok_or_else(|| "saved OSC command address was missing".to_string())?;

    let input_value = match param_value(engine, input) {
        Some(ParamValue::Int(value)) => value,
        other => return Err(format!("saved OSC input value was not Int(42): {other:?}")),
    };
    let command_address = match param_value(engine, address) {
        Some(ParamValue::Str(value)) => value,
        other => return Err(format!("saved OSC command address was not text: {other:?}")),
    };
    Ok(json!({
        "module_type": GenericOscModule::NODE_TYPE,
        "input_address": INPUT_ADDRESS,
        "input_value": input_value,
        "command_address": command_address,
    }))
}

fn available_udp_port() -> Result<u16, String> {
    UdpSocket::bind("127.0.0.1:0")
        .and_then(|socket| socket.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("failed to reserve an OSC evidence input port: {error}"))
}
