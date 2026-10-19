use super::*;
use golden_core::{
    edit::Edit,
    node::Folder,
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::time::Duration;

fn pool(items: usize, bytes: usize) -> PayloadPool {
    PayloadPool::new("native-callback-test", items, bytes).unwrap()
}

#[test]
fn ordinary_event_copies_keep_the_payload_reservation_after_preparation_is_gone() {
    let pool = pool(1, 64 * 1024);
    let prepared = NativeCallbackPayload::prepare_in(&pool, "received", vec![Value::from("body")]).unwrap();
    let event = prepared.event(NodeId(1));
    let routed = event.clone();
    assert!(Arc::ptr_eq(&prepared.payload, &routed.payload));
    drop(prepared);
    assert!(pool.try_retain(1u8, 1).is_err());
    drop(event);
    assert!(pool.try_retain(1u8, 1).is_err());
    drop(routed);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(NativeCallbackPayload::prepare_in(&pool, "recovered", vec![]).is_ok());
}

#[test]
fn replay_and_exported_events_retain_the_same_allocation_and_release_capacity() {
    let pool = pool(1, 64 * 1024);
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let prepared = NativeCallbackPayload::prepare_in(&pool, "received", vec![Value::from("body")]).unwrap();
    let weak = Arc::downgrade(&prepared.payload);
    engine.edits.push(Edit::EmitCustomEvent {
        event: prepared.event(engine.root),
    });
    drop(prepared);
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    let exported = engine.ui_events_since(None);
    assert_eq!(exported.len(), 1);
    assert!(pool.try_retain(1u8, 1).is_err());
    engine.clear_ui_event_log();
    assert!(pool.try_retain(1u8, 1).is_err());
    drop(engine);
    assert!(pool.try_retain(1u8, 1).is_err());
    drop(exported);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(weak.upgrade().is_none());
}

#[test]
fn failed_publication_retry_does_not_release_or_duplicate_the_callback() {
    use crate::app::module::script_api::CallbackDelivery;
    use golden_core::{
        node::NodeId,
        parameter::{ParamValue, ParameterEventBehaviour},
    };
    let pool = pool(1, 64 * 1024);
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let prepared = NativeCallbackPayload::prepare_in(&pool, "received", vec![]).unwrap();
    let mut delivery = CallbackDelivery::new(prepared.event(engine.root));
    drop(prepared);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(!delivery.service(&mut ctx));
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert!(engine.ui_event_log().is_empty());
    assert!(pool.try_retain(1u8, 1).is_err());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(!delivery.service(&mut ctx));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(delivery.service(&mut ctx));
    drop(delivery);
    assert_eq!(engine.ui_event_log().len(), 1);
    assert!(pool.try_retain(1u8, 1).is_err());
    engine.clear_ui_event_log();
    drop(engine);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn byte_admission_counts_spare_string_and_array_storage() {
    let pool = pool(4, 32 * 1024);
    let mut text = String::with_capacity(64 * 1024);
    text.push('x');
    assert!(
        NativeCallbackPayload::prepare_in(&pool, "received", vec![Value::String(text)])
            .unwrap_err()
            .contains("capacity exhausted")
    );
    let mut args = Vec::with_capacity(4096);
    args.push(Value::Null);
    assert!(NativeCallbackPayload::prepare_in(&pool, "received", args)
        .unwrap_err()
        .contains("capacity exhausted"));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn maximum_payload_storage_is_rejected_before_pool_admission() {
    let pool = pool(4, 64 * 1024 * 1024);
    let text = String::with_capacity(MAX_CALLBACK_BYTES);
    assert!(
        NativeCallbackPayload::prepare_in(&pool, "received", vec![Value::String(text)])
            .unwrap_err()
            .contains("32 MiB")
    );
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn excessive_callback_complexity_is_rejected_without_occupying_capacity() {
    let pool = pool(4, 64 * 1024 * 1024);
    let mut nested = Value::Null;
    for _ in 0..MAX_CALLBACK_DEPTH {
        nested = Value::Array(vec![nested]);
    }
    assert!(NativeCallbackPayload::prepare_in(&pool, "received", vec![nested])
        .unwrap_err()
        .contains("64 levels"));
    let values = Value::Array(vec![Value::Null; MAX_CALLBACK_VALUES]);
    assert!(NativeCallbackPayload::prepare_in(&pool, "received", vec![values])
        .unwrap_err()
        .contains("262144 values"));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn binding_keeps_callback_shape_origin_retention_and_cached_storage() {
    let pool = pool(1, 64 * 1024);
    let prepared = NativeCallbackPayload::prepare_in(&pool, "received", vec![Value::from(7)]).unwrap();
    let event = prepared.event(NodeId(42));
    assert_eq!(event.origin, Some(NodeId(42)));
    assert_eq!(event.topic, super::super::MODULE_SCRIPT_CALLBACK_TOPIC);
    assert_eq!(event.retention, CustomEventRetention::Replay);
    assert_eq!(event.payload["callback"], "received");
    assert_eq!(event.payload["args"], serde_json::json!([7]));
    assert!(Arc::ptr_eq(&prepared.payload, &event.payload));
    assert!(prepared.retained_bytes() >= size_of::<Value>());
}
