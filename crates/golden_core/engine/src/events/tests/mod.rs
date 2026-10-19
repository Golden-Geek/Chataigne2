use super::*;

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
