use super::*;
use golden_io::PayloadPool;
use std::time::Duration;

#[test]
fn event_object_preparation_preserves_nested_string_allocations() {
    let text = "payload".repeat(32);
    let pointer = text.as_ptr();
    let mut object = serde_json::Map::with_capacity(16_384);
    object.insert("value".into(), Value::String(text));
    let event = ScriptCustomEvent::prepare("topic", Value::Object(object)).unwrap();
    assert_eq!(event.payload()["value"].as_str().unwrap().as_ptr(), pointer);
}

#[test]
fn event_raw_routing_owners_keep_original_storage_and_reservation_after_bridge_drop() {
    let pool = PayloadPool::new("script-event-test", 1, MAX_INPUT_BYTES).unwrap();
    let reservation = ScriptEventReservation {
        topic: "event".into(),
        permit: pool.try_reserve(MAX_INPUT_BYTES).unwrap(),
    };
    let event = reservation.finish(serde_json::json!({"value":[1,2,3]})).unwrap();
    let routed = event.payload().clone();
    let pointer = Arc::as_ptr(&routed);
    let replay = event.clone();
    drop(event);
    let (topic, payload) = replay.into_parts();
    assert_eq!(topic, "event");
    assert_eq!(Arc::as_ptr(&payload), pointer);
    drop(payload);
    assert!(!pool.wait_for_idle(Duration::ZERO));
    assert!(pool.try_reserve(1).is_err());
    drop(routed);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.try_reserve(MAX_INPUT_BYTES).is_ok());
}

#[test]
fn event_preparation_rejects_retained_capacity_and_wide_shapes_before_publication() {
    for payload in [
        Value::String(String::with_capacity(MAX_INPUT_BYTES)),
        Value::Array(Vec::with_capacity(MAX_INPUT_BYTES / std::mem::size_of::<Value>() + 1)),
        Value::Array(vec![Value::Null; 4097]),
    ] {
        let error = ScriptCustomEvent::prepare("topic", payload).unwrap_err();
        let (topic, _) = error.into_input();
        assert_eq!(topic, "topic");
    }
}
