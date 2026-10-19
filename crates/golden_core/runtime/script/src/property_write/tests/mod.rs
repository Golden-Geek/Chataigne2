use super::*;
use golden_io::PayloadPool;
use std::time::Duration;

#[test]
fn property_preparation_preserves_storage_and_returns_rejected_original_values() {
    let mut text = String::with_capacity(4096);
    text.push('x');
    let pointer = text.as_ptr();
    let write = ScriptPropertyWrite::prepare("value", ParamValue::Str(text)).unwrap();
    let copy = write.clone();
    drop(write);
    let ParamValue::Str(text) = copy.value() else {
        panic!("string property")
    };
    assert_eq!(text.as_ptr(), pointer);
    assert_eq!(text.capacity(), 4096);
    let original = String::with_capacity(MAX_INPUT_BYTES);
    let pointer = original.as_ptr();
    let (_, returned) = ScriptPropertyWrite::prepare("value", ParamValue::Str(original))
        .unwrap_err()
        .into_input();
    let ParamValue::Str(returned) = returned else {
        panic!("original string")
    };
    assert_eq!(returned.as_ptr(), pointer);
}

#[test]
fn property_reservation_retains_capacity_until_all_queued_and_captured_leases_are_released() {
    let pool = PayloadPool::new("property-input-test", 1, MAX_INPUT_BYTES).unwrap();
    let reservation = ScriptPropertyReservation {
        property: "value".into(),
        permit: pool.try_reserve(MAX_INPUT_BYTES).unwrap(),
    };
    assert!(pool.try_reserve(1).is_err());
    let write = reservation.finish(ParamValue::Str("original".into())).unwrap();
    let captured = write.clone();
    drop(write);
    assert!(!pool.wait_for_idle(Duration::ZERO));
    drop(captured);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
