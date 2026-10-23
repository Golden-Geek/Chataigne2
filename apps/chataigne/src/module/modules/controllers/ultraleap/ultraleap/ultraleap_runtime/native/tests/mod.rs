use super::super::UltraleapVec3;
use super::*;
#[test]
fn native_endpoint_cannot_open_in_test_builds() {
    assert!(create().is_err());
}
#[test]
fn malformed_hand_counts_and_null_arrays_reject_before_pointer_traversal() {
    let mut event: leap_sys::LEAP_TRACKING_EVENT = unsafe { std::mem::zeroed() };
    event.nHands = u32::MAX;
    assert!(tracking::snapshot_from_tracking(&event).is_err());
    event.nHands = 1;
    assert!(tracking::snapshot_from_tracking(&event).is_err());
    event.nHands = 0;
    assert_eq!(tracking::snapshot_from_tracking(&event).unwrap().hand_count, 0);
}
#[test]
fn sdk_hand_conversion_uses_meters_and_rejects_duplicate_identity() {
    let mut hands: [leap_sys::LEAP_HAND; 2] = unsafe { std::mem::zeroed() };
    hands[0].type_ = leap_sys::_eLeapHandType_eLeapHandType_Left;
    hands[0].palm.position.__bindgen_anon_1.v = [100.0, 200.0, 300.0];
    let mut event: leap_sys::LEAP_TRACKING_EVENT = unsafe { std::mem::zeroed() };
    event.nHands = 1;
    event.pHands = hands.as_mut_ptr();
    let frame = tracking::snapshot_from_tracking(&event).unwrap();
    assert_eq!(frame.left.palm_position, UltraleapVec3::new(0.1, 0.2, 0.3));
    event.nHands = 2;
    assert!(tracking::snapshot_from_tracking(&event).is_err());
}
