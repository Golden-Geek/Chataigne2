use super::super::super::{
    output::{Method, Request},
    tests::{create_mqtt_module, find_path},
};
use super::*;
use golden_core::{
    edit::Edit,
    events::CustomEvent,
    node::{Folder, Node},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptMethodCall,
};

fn source(engine: &mut crate::app::AppEngine, producer: NodeId, method: Method, payload: ParamValue) -> Request {
    let call = ScriptMethodCall::prepare(
        method.name(),
        vec![
            ParamValue::Str("test/value".into()),
            payload,
            ParamValue::Int(1),
            ParamValue::Bool(true),
        ],
    )
    .unwrap();
    Request::script_arguments(method, call.args().clone(), engine.node_waker(producer).unwrap()).unwrap()
}

#[test]
fn captured_output_keeps_its_quota_until_the_original_acknowledgement() {
    let mut engine = crate::app::AppEngine::new(Folder::new("source").into());
    let producer = engine.root;
    let control = Control::default();
    let lifetimes = slots();
    let output = pool(2); // Configuration and one publication.
    let mut handle = fake(control.clone(), &lifetimes, output.clone(), pool(8));
    let receipt = handle
        .send_output(
            source(&mut engine, producer, Method::Json, ParamValue::Vec2(1.5, 2.5)),
            false,
        )
        .unwrap();
    until(|| control.published.load(Ordering::Acquire) == 1);
    let packet = control.last_request.lock().unwrap().clone().unwrap();
    assert_eq!(packet.payload, b"[1.5,2.5]");
    assert_eq!(packet.qos, MqttQos::AtLeast);
    assert!(packet.retain);
    assert!(handle
        .send_output(
            source(&mut engine, producer, Method::Text, ParamValue::Str("pressure".into())),
            false
        )
        .is_err());
    assert_eq!(receipt.status(), PublishStatus::Pending);
    control.event(Event::Incoming(Packet::PubAck(rumqttc::PubAck { pkid: 1 })));
    until(|| receipt.status() == PublishStatus::Acknowledged);
    let mut second = None;
    until(|| {
        second = handle
            .send_output(
                source(&mut engine, producer, Method::Text, ParamValue::Str("recovered".into())),
                false,
            )
            .ok();
        second.is_some()
    });
    until(|| control.published.load(Ordering::Acquire) == 2);
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(second.unwrap().status(), PublishStatus::FailedUnconfirmed);
}

#[test]
fn rejected_preparation_never_dispatches_an_empty_packet_and_later_output_recovers() {
    let mut engine = crate::app::AppEngine::new(Folder::new("source").into());
    let producer = engine.root;
    let control = Control::default();
    let lifetimes = slots();
    let mut handle = fake(control.clone(), &lifetimes, pool(8), pool(8));
    let call = ScriptMethodCall::prepare("publish", vec![ParamValue::Str("invalid/#".into())]).unwrap();
    let bad_topic = Request::script_arguments(
        Method::Publish,
        call.args().clone(),
        engine.node_waker(producer).unwrap(),
    )
    .unwrap();
    let receipt = handle.send_output(bad_topic, false).unwrap();
    until(|| receipt.status() != PublishStatus::Pending);
    assert_eq!(receipt.status(), PublishStatus::Rejected);
    assert!(receipt.failure().unwrap().contains("topic"));
    let expansion = handle
        .send_output(
            source(
                &mut engine,
                producer,
                Method::Json,
                ParamValue::Str("\0".repeat(300_000)),
            ),
            false,
        )
        .unwrap();
    until(|| expansion.status() != PublishStatus::Pending);
    assert_eq!(expansion.status(), PublishStatus::Rejected);
    assert!(expansion.failure().unwrap().contains("encoding"));
    assert_eq!(control.published.load(Ordering::Acquire), 0);
    let next = handle
        .send_output(
            source(&mut engine, producer, Method::Text, ParamValue::Str("valid".into())),
            false,
        )
        .unwrap();
    until(|| control.published.load(Ordering::Acquire) == 1);
    control.event(Event::Incoming(Packet::PubAck(rumqttc::PubAck { pkid: 1 })));
    until(|| next.status() == PublishStatus::Acknowledged);
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn retired_producer_cancels_an_already_prepared_busy_publication() {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    engine.add_node(Folder::new("producer").into(), None);
    engine.apply_edits().unwrap();
    let producer = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let control = Control::default();
    control.busy.store(true, Ordering::Release);
    let lifetimes = slots();
    let output = pool(8);
    let mut handle = fake(control.clone(), &lifetimes, output.clone(), pool(8));
    let receipt = handle
        .send_output(
            source(&mut engine, producer, Method::Text, ParamValue::Str("stale".into())),
            false,
        )
        .unwrap();
    until(|| control.attempts.load(Ordering::Acquire) > 0);
    engine.edits.push(Edit::RemoveNode { node: producer });
    engine.apply_edits().unwrap();
    until(|| receipt.status() != PublishStatus::Pending);
    control.busy.store(false, Ordering::Release);
    assert_eq!(receipt.status(), PublishStatus::Rejected);
    assert!(receipt.failure().unwrap().contains("retired"));
    assert_eq!(control.published.load(Ordering::Acquire), 0);
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn engine_script_methods_and_command_execution_reach_the_same_native_boundary() {
    let (mut engine, module_id) = create_mqtt_module();
    let parent = find_path(&engine, module_id, "command_tester").unwrap();
    engine.add_user_item(crate::app::MqttPublishCommand::create().into(), Some(parent));
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    let command = engine.nodes.get(parent).unwrap().node_data().first_child.unwrap();
    let payload = find_path(&engine, command, "payload").unwrap();
    let trigger = find_path(&engine, command, "trigger").unwrap();
    let control = Control::default();
    let lifetimes = slots();
    let handle = fake(control.clone(), &lifetimes, pool(16), pool(8));
    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        unreachable!()
    };
    module.transport = Some(handle);
    for (index, method) in ["publish", "publishText", "publishJson"].into_iter().enumerate() {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: module_id,
            call: ScriptMethodCall::prepare(method, vec![ParamValue::Str("test".into()), ParamValue::Int(42)]).unwrap(),
        });
        engine.apply_edits().unwrap();
        until(|| control.published.load(Ordering::Acquire) == index + 1);
        assert_eq!(control.last_request.lock().unwrap().as_ref().unwrap().payload, b"42");
    }
    engine.edits.push(Edit::SetParam {
        node: payload,
        value: ParamValue::Str(r"manual\n".into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: trigger,
        value: ParamValue::Trigger(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    until(|| control.published.load(Ordering::Acquire) == 4);
    assert_eq!(
        control.last_request.lock().unwrap().as_ref().unwrap().payload,
        b"manual\n"
    );
    let event = CustomEvent::new(
        crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
        Some(command),
        serde_json::json!({ "command_id": command.0, "param_overrides": [{"param_id": payload.0, "value": ParamValue::Str("override".into())}] }),
    );
    let mut ctx = golden_core::process_ctx::ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.emit_custom_event(event);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    until(|| control.published.load(Ordering::Acquire) == 5);
    assert_eq!(
        control.last_request.lock().unwrap().as_ref().unwrap().payload,
        b"override"
    );
    let crate::app::AppNode::MqttModule(module) = engine.nodes.get_mut(module_id).unwrap() else {
        unreachable!()
    };
    module.stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
