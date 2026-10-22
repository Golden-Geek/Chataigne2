use super::super::{
    tests::{create_mqtt_module, find_path},
    transport::MqttTransportHandle,
};
use super::*;
use crate::app::module::{common::mqtt::MqttQos, script_api::NativeCallbackPreparer};
use golden_core::{
    edit::Edit,
    events::EventKind,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use golden_io::PayloadPool;

mod delivery;
mod preparation;

fn raw(topic: &str, payload: &[u8]) -> RetainedPayload<MqttReceivedPublish> {
    MqttTransportHandle::retain_for_test(MqttReceivedPublish {
        topic: topic.into(),
        payload: payload.to_vec(),
        qos: MqttQos::AtLeast,
        retain: true,
    })
}
fn source(engine: &crate::app::AppEngine, id: NodeId, lifetime: &RevisionSource) -> Source {
    Source::capture(&engine.process_tree_snapshot(), id, lifetime.reader().guard_at(0)).unwrap()
}
fn prepared(
    engine: &crate::app::AppEngine,
    id: NodeId,
    lifetime: &RevisionSource,
    topic: &str,
    payload: &[u8],
) -> RetainedPayload<Prepared> {
    let mut draft = build::Draft::new(raw(topic, payload), source(engine, id, lifetime), false).unwrap();
    draft
        .retain(
            &PayloadPool::new("mqtt-input-test", 8, 32 * 1024 * 1024).unwrap(),
            &NativeCallbackPreparer::new().unwrap(),
        )
        .unwrap()
}
fn callbacks(engine: &crate::app::AppEngine) -> usize {
    engine.inbox.events.iter().filter(|event| matches!(&event.kind, EventKind::Custom(custom) if custom.topic == crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC)).count()
}
fn tick(engine: &mut crate::app::AppEngine, delivery: &mut Delivery) -> (bool, ProcessCtx) {
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let done = delivery.service(&mut ctx, Some(&snapshot));
    (done, ctx)
}

#[test]
fn worker_preparation_preserves_callback_arguments_and_json_paths() {
    let (engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let input = prepared(
        &engine,
        id,
        &lifetime,
        "devices/kitchen",
        br#"{"temperature":21,"humidity":0.42,"rgb":[1.0,0.5,0.0,1.0]}"#,
    );
    assert_eq!(input.values.len(), 3);
    assert_eq!(input.callback.payload()["callback"], "messageReceived");
    assert_eq!(input.callback.payload()["args"][0], "devices/kitchen");
    assert_eq!(input.callback.payload()["args"][2]["qos"], "at_least_once");
    assert_eq!(input.callback.payload()["args"][2]["retain"], true);
}

#[test]
fn expansion_limits_reject_before_large_value_and_callback_materialization() {
    let (engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    for (topic, bytes, mode) in [
        ("large".to_owned(), vec![b'x'; MAX_PAYLOAD + 1], PayloadMode::Text),
        ("x/".repeat(25), vec![b'x'], PayloadMode::Text),
        ("raw".to_owned(), vec![0; MAX_UNITS + 1], PayloadMode::Raw),
        (
            "json".to_owned(),
            format!("[{}]", "0,".repeat(9000).trim_end_matches(',')).into_bytes(),
            PayloadMode::Json,
        ),
    ] {
        let mut source = source(&engine, id, &lifetime);
        source.mode.payload = mode;
        let result = build::Draft::new(raw(&topic, &bytes), source, false);
        if bytes.len() > MAX_PAYLOAD {
            assert!(result.is_err());
        } else {
            let input = result
                .unwrap()
                .retain(
                    &PayloadPool::new("mqtt-rejected-values", 1, 32 * 1024 * 1024).unwrap(),
                    &NativeCallbackPreparer::new().unwrap(),
                )
                .unwrap();
            assert!(input.values.is_empty());
            assert!(
                input.error.is_some(),
                "unconverted values must have an explicit outcome while the raw callback survives"
            );
        }
    }
}
