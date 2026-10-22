use super::*;
use crate::app::{
    module_command::{ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride},
    module_modules_protocol_mqtt::tests::{create_mqtt_module, find_path},
};
use golden_core::{
    edit::Edit,
    events::CustomEvent,
    node::{Folder, Node, NodeId},
    parameter::ParameterEventBehaviour,
    script::ScriptMethodCall,
};

fn command_fixture() -> (crate::app::AppEngine, NodeId, NodeId) {
    let (mut engine, module) = create_mqtt_module();
    let parent = find_path(&engine, module, "command_tester").unwrap();
    engine.add_user_item(crate::app::MqttPublishCommand::create().into(), Some(parent));
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    let command = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    (engine, module, command)
}

fn set(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}

#[test]
fn command_capture_freezes_authored_text_and_decodes_escapes_on_preparation() {
    let (mut engine, module, command) = command_fixture();
    let payload = find_path(&engine, command, "payload").unwrap();
    set(&mut engine, payload, ParamValue::Str(r"before\n\x41".into()));
    let snapshot = engine.process_tree_snapshot();
    let invocation = CapturedInvocation::capture(&snapshot, command, None, &["mqtt_module"]).unwrap();
    assert_eq!(invocation.module, module);
    let source = Request::command(invocation, engine.node_waker(command).unwrap());
    set(&mut engine, payload, ParamValue::Str("after".into()));
    let prepared = source.prepare().unwrap();
    assert_eq!(prepared.payload, b"before\nA");
    assert_eq!(prepared.topic, "chataigne/topic");
}

#[test]
fn command_execution_keeps_last_override_without_changing_captured_parameters() {
    let (engine, _, command) = command_fixture();
    let payload = find_path(&engine, command, "payload").unwrap();
    let snapshot = engine.process_tree_snapshot();
    let event = ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: payload,
                value: ParamValue::Str("first".into()),
            },
            ModuleCommandParamOverride {
                param_id: payload,
                value: ParamValue::Str(r"last\tvalue".into()),
            },
        ],
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    let invocation = CapturedInvocation::capture(
        &snapshot,
        command,
        Some(CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(event).unwrap(),
        )),
        &["mqtt_module"],
    )
    .unwrap();
    let source = Request::command(invocation, engine.node_waker(command).unwrap());
    assert_eq!(source.prepare().unwrap().payload, b"last\tvalue");
    assert_eq!(
        snapshot.node(payload).unwrap().param_value,
        Some(ParamValue::Str(String::new()))
    );
}

#[test]
fn script_capture_shares_original_arguments_and_rejects_extra_arguments() {
    let engine = crate::app::AppEngine::new(Folder::new("source").into());
    let call = ScriptMethodCall::prepare(
        "publishText",
        vec![ParamValue::Str("test".into()), ParamValue::Str("shared".into())],
    )
    .unwrap();
    let source = Request::script_arguments(
        Method::Text,
        call.args().clone(),
        engine.node_waker(engine.root).unwrap(),
    )
    .unwrap();
    let Source::Script { args, .. } = &source.source else {
        unreachable!()
    };
    assert_eq!(args.as_ptr(), call.args().as_ptr());
    assert_eq!(source.prepare().unwrap().payload, b"shared");
    let extra = ScriptMethodCall::prepare("publish", vec![ParamValue::Int(1); 5]).unwrap();
    assert!(Request::script_arguments(
        Method::Publish,
        extra.args().clone(),
        engine.node_waker(engine.root).unwrap()
    )
    .is_err());
}

#[test]
fn json_writer_rejects_encoded_expansion_and_oversized_topic_before_a_packet_exists() {
    let args = [
        ParamValue::Str("test".into()),
        ParamValue::Str("\0".repeat(MAX_PACKET / 2)),
    ];
    assert!(script::prepare(Method::Json, &args, MAX_PACKET)
        .unwrap_err()
        .contains("exceeds"));
    let args = [ParamValue::Str("x".repeat(MAX_TOPIC + 1))];
    assert!(script::prepare(Method::Text, &args, MAX_PACKET)
        .unwrap_err()
        .contains("topic"));
}
