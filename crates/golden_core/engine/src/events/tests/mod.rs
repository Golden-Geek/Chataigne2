use super::*;

#[test]
fn prepared_storage_cache_is_runtime_only_and_invalidates_after_payload_changes() {
    let mut event = CustomEvent::new("weighted", None, serde_json::json!("small")).with_payload_storage_weight(4096);
    assert_eq!(event.replay_payload_bytes(), 4096);
    assert_eq!(event.clone().replay_payload_bytes(), 4096);
    let encoded = serde_json::to_value(&event).unwrap();
    assert!(encoded.get("payload_storage").is_none());
    let decoded: CustomEvent = serde_json::from_value(encoded).unwrap();
    assert_eq!(event, decoded);
    assert!(decoded.replay_payload_bytes() < 4096);

    *Arc::make_mut(&mut event.payload) = serde_json::json!("changed");
    assert!(event.replay_payload_bytes() < 4096);
    event = event.with_payload_storage_weight(8192);
    event.payload = Arc::new(serde_json::Value::Null);
    assert_eq!(
        event.replay_payload_bytes(),
        std::mem::size_of::<serde_json::Value>() + 2 * std::mem::size_of::<usize>()
    );
}

#[test]
fn unprepared_storage_walk_counts_spare_capacity_and_bounds_complexity() {
    let mut text = String::with_capacity(8192);
    text.push('x');
    let event = CustomEvent::new("spare", None, serde_json::Value::String(text));
    assert!(event.replay_payload_bytes() >= 8192);
    let complex = CustomEvent::new(
        "complex",
        None,
        serde_json::Value::Array(vec![serde_json::Value::Null; 4096]),
    );
    assert_eq!(complex.replay_payload_bytes(), usize::MAX);
}

#[test]
fn shared_event_capture_reuses_canonical_owner_and_survives_serialization_without_metadata_drift() {
    use golden_io::PayloadPool;
    use std::time::Duration;
    let pool = PayloadPool::new("event-lease-test", 1, 1024).unwrap();
    let payload = pool
        .try_reserve(1024)
        .unwrap()
        .share_retained(serde_json::json!({"value":42}), 512)
        .unwrap();
    let raw = payload.arc().clone();
    let event = CustomEvent::from_shared_payload("topic", None, payload, CustomEventRetention::Replay);
    // An independent pool could not admit this weight; the existing owner is reused.
    let lease = event.try_lease_payload(usize::MAX).unwrap();
    assert!(std::ptr::eq(&*lease, raw.as_ref()));
    let encoded = serde_json::to_value(&event).unwrap();
    assert!(encoded.get("payload_owner").is_none());
    let decoded: CustomEvent = serde_json::from_value(encoded).unwrap();
    assert_eq!(event, decoded);
    drop((event, lease, decoded));
    assert!(pool.try_reserve(1).is_err());
    drop(raw);
    assert!(
        pool.wait_for_idle(Duration::from_secs(2)),
        "canonical pool must recover without an ownership cycle"
    );
}

#[test]
fn replacing_public_event_payload_does_not_capture_its_obsolete_owner() {
    let pool = golden_io::PayloadPool::new("event-replaced", 1, 1024).unwrap();
    let payload = pool
        .try_reserve(1024)
        .unwrap()
        .share_retained(serde_json::json!(1), 64)
        .unwrap();
    let mut event = CustomEvent::from_shared_payload("topic", None, payload, CustomEventRetention::Replay);
    event.payload = Arc::new(serde_json::json!(2));
    assert_eq!(*event.try_lease_payload(64).unwrap(), serde_json::json!(2));
}

#[test]
fn local_payload_is_shared_typed_and_never_restored_from_json() {
    let event = CustomEvent::local("runtime.payload", None, vec![1_u8, 2, 3]);
    let copy = event.clone();
    assert!(std::ptr::eq(
        event.local_payload::<Vec<u8>>().unwrap(),
        copy.local_payload::<Vec<u8>>().unwrap()
    ));
    assert!(event.local_payload::<String>().is_none());
    let encoded = serde_json::to_value(&event).unwrap();
    assert!(encoded.get("local").is_none());
    let decoded: CustomEvent = serde_json::from_value(encoded).unwrap();
    assert!(decoded.local_payload::<Vec<u8>>().is_none());
    assert!(decoded.payload.is_null());
}

#[test]
fn local_payload_stays_transient_when_public_retention_metadata_changes() {
    let mut event = CustomEvent::local("runtime.payload", None, 7_u32);
    event.retention = CustomEventRetention::Replay;
    assert_eq!(event.effective_retention(), CustomEventRetention::Transient);
    let json = Arc::new(serde_json::json!({"value": 9}));
    let shared = CustomEvent::shared("json", None, json.clone(), CustomEventRetention::Replay);
    assert!(Arc::ptr_eq(&json, &shared.payload));
    assert_eq!(shared.effective_retention(), CustomEventRetention::Replay);
}
