use super::*;
use golden_core::parameter::ParamValue;
use std::time::{Duration, Instant};

mod delivery;

fn message(address: &str) -> OscDecodedMessage {
    OscDecodedMessage {
        address: address.into(),
        payload: OscValuePayload::Single(ParamValue::Int(42)),
    }
}

fn isolated(value_items: usize, callback_items: usize) -> InputPreparer {
    InputPreparer {
        plan: InputPlan::new(NodeId(1), true),
        values: PayloadPool::new("osc-test-values", value_items, 4 * 1024 * 1024).unwrap(),
        callbacks: NativeCallbackPreparer::using(
            PayloadPool::new("osc-test-callbacks", callback_items, 4 * 1024 * 1024).unwrap(),
        ),
    }
}

#[test]
fn prepared_input_preserves_callback_arguments_and_value_paths() {
    let preparer = isolated(1, 1);
    let mut draft = preparer.draft(message("/rig/ gain /")).unwrap();
    let prepared = preparer.retain(&mut draft).unwrap();
    assert_eq!(prepared.values[0].path_segments, ["rig", "gain"]);
    assert_eq!(
        prepared.values[0].payload,
        ReceivedValuePayload::Single(ParamValue::Int(42))
    );
    assert!(prepared.log.as_ref().unwrap().starts_with("Received OSC /rig/ gain /"));
    let payload = prepared.callbacks[0].payload();
    assert_eq!(payload["callback"], "messageReceived");
    assert_eq!(
        payload["args"],
        serde_json::json!(["/rig/ gain /", 42, {"address":"/rig/ gain /","payload":42}])
    );
}

#[test]
fn incoming_batch_admits_sixteen_callback_occurrences_and_rejects_seventeen() {
    let preparer = isolated(1, 16);
    let mut draft = preparer
        .draft_batch((0..16).map(|index| message(&format!("/batch/{index}"))).collect())
        .unwrap();
    let prepared = preparer.retain(&mut draft).unwrap();
    assert_eq!(prepared.values.len(), 16);
    assert_eq!(prepared.callbacks.len(), 16);
    for (index, callback) in prepared.callbacks.iter().enumerate() {
        assert_eq!(callback.payload()["args"][0], format!("/batch/{index}"));
    }
    assert!(preparer
        .draft_batch((0..17).map(|_| message("/overflow")).collect())
        .is_err());
}

#[test]
fn invalid_incoming_shape_is_rejected_before_retention() {
    let preparer = isolated(1, 1);
    for address in [
        "missing/slash".into(),
        format!("/{}", "x".repeat(4096)),
        "/x".repeat(65),
        "/bad\0address".into(),
    ] {
        assert!(preparer.draft(message(&address)).is_err());
    }
    let mut large = message("/large");
    large.payload = OscValuePayload::Multi(vec![ParamValue::Int(1); 1025]);
    assert!(preparer.draft(large).is_err());
}

#[test]
fn full_value_pool_keeps_the_same_prepared_input_until_capacity_recovers() {
    let preparer = isolated(1, 2);
    let mut first = preparer.draft(message("/first")).unwrap();
    let retained = preparer.retain(&mut first).unwrap();
    drop(first);
    let mut second = preparer.draft(message("/second")).unwrap();
    assert!(preparer.retain(&mut second).is_none());
    let pointer = Arc::as_ptr(second.prepared.as_ref().unwrap());
    for _ in 0..10 {
        assert!(preparer.retain(&mut second).is_none());
        assert_eq!(Arc::as_ptr(second.prepared.as_ref().unwrap()), pointer);
    }
    drop(retained);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(prepared) = preparer.retain(&mut second) {
            assert_eq!(prepared.values[0].path_segments, ["second"]);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn full_callback_pool_retains_the_unpublished_draft_without_rebuilding_it() {
    let preparer = isolated(2, 1);
    let mut first = preparer.draft(message("/first")).unwrap();
    let retained = preparer.retain(&mut first).unwrap();
    drop(first);
    let mut second = preparer.draft(message("/second")).unwrap();
    assert!(preparer.retain(&mut second).is_none());
    assert!(second.prepared.is_none());
    assert_eq!(second.values[0].path_segments, ["second"]);
    drop(retained);
    let deadline = Instant::now() + Duration::from_secs(3);
    while preparer.retain(&mut second).is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
