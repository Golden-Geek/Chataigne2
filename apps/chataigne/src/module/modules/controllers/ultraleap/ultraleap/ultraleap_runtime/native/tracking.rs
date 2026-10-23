use super::super::{UltraleapFrameSnapshot, UltraleapHandSnapshot, UltraleapVec3};
use std::ptr;
const MILLIMETERS_PER_METER: f64 = 1000.0;
#[derive(Clone, Copy)]
enum UltraleapHandSide {
    Left,
    Right,
}

pub(super) fn message_event_type(message: &leap_sys::LEAP_CONNECTION_MESSAGE) -> leap_sys::eLeapEventType {
    unsafe { ptr::read_unaligned(ptr::addr_of!(message.type_)) }
}

pub(super) fn tracking_event_from_message(
    message: &leap_sys::LEAP_CONNECTION_MESSAGE,
) -> Option<*const leap_sys::LEAP_TRACKING_EVENT> {
    let event = unsafe { ptr::read_unaligned(ptr::addr_of!(message.__bindgen_anon_1.tracking_event)) };
    if event.is_null() {
        None
    } else {
        Some(event)
    }
}

pub(super) fn snapshot_from_tracking(
    event: *const leap_sys::LEAP_TRACKING_EVENT,
) -> Result<UltraleapFrameSnapshot, String> {
    let mut snapshot = UltraleapFrameSnapshot::default();
    if event.is_null() {
        return Ok(snapshot);
    }

    let event = unsafe { ptr::read_unaligned(event) };
    let hand_count = unsafe { ptr::read_unaligned(ptr::addr_of!(event.nHands)) as usize };
    let hands = unsafe { ptr::read_unaligned(ptr::addr_of!(event.pHands)) as *const leap_sys::LEAP_HAND };

    if hand_count > 2 {
        return Err("Ultraleap tracking exceeds two hands".into());
    }
    snapshot.hand_count = hand_count;

    if hands.is_null() {
        return if hand_count == 0 {
            Ok(snapshot)
        } else {
            Err("Ultraleap hands pointer is null".into())
        };
    }

    for index in 0..hand_count {
        let hand = unsafe { hands.add(index) };
        match hand_side(hand) {
            Some(UltraleapHandSide::Left) if !snapshot.left.active => snapshot.left = snapshot_from_hand(hand),
            Some(UltraleapHandSide::Right) if !snapshot.right.active => snapshot.right = snapshot_from_hand(hand),
            _ => return Err("Ultraleap hand identity is invalid or duplicated".into()),
        }
    }

    Ok(snapshot)
}

fn hand_side(hand: *const leap_sys::LEAP_HAND) -> Option<UltraleapHandSide> {
    if hand.is_null() {
        return None;
    }

    let hand = unsafe { ptr::read_unaligned(hand) };
    match unsafe { ptr::read_unaligned(ptr::addr_of!(hand.type_)) } {
        leap_sys::_eLeapHandType_eLeapHandType_Left => Some(UltraleapHandSide::Left),
        leap_sys::_eLeapHandType_eLeapHandType_Right => Some(UltraleapHandSide::Right),
        _ => None,
    }
}

fn snapshot_from_hand(hand: *const leap_sys::LEAP_HAND) -> UltraleapHandSnapshot {
    let hand = unsafe { ptr::read_unaligned(hand) };
    let palm = unsafe { ptr::read_unaligned(ptr::addr_of!(hand.palm)) };
    let digits = unsafe { ptr::read_unaligned(ptr::addr_of!(hand.__bindgen_anon_1.digits)) };

    UltraleapHandSnapshot {
        active: true,
        grab_strength: unsafe { ptr::read_unaligned(ptr::addr_of!(hand.grab_strength)) as f64 },
        pinch_strength: unsafe { ptr::read_unaligned(ptr::addr_of!(hand.pinch_strength)) as f64 },
        pinch_distance: millimeters_to_meters(unsafe {
            ptr::read_unaligned(ptr::addr_of!(hand.pinch_distance)) as f64
        }),
        thumb_extended: digit_is_extended(&digits, 0),
        index_extended: digit_is_extended(&digits, 1),
        middle_extended: digit_is_extended(&digits, 2),
        ring_extended: digit_is_extended(&digits, 3),
        pinky_extended: digit_is_extended(&digits, 4),
        palm_position: leap_vec3_meters(ptr::addr_of!(palm.position)),
        palm_stabilized_position: leap_vec3_meters(ptr::addr_of!(palm.stabilized_position)),
        palm_velocity: leap_vec3_meters(ptr::addr_of!(palm.velocity)),
        palm_direction: leap_vec3(ptr::addr_of!(palm.direction)),
        palm_normal: leap_vec3(ptr::addr_of!(palm.normal)),
    }
}

fn digit_is_extended(digits: &[leap_sys::LEAP_DIGIT; 5], index: usize) -> bool {
    let digit = digits.as_ptr().wrapping_add(index);
    unsafe { ptr::read_unaligned(ptr::addr_of!((*digit).is_extended)) != 0 }
}

fn leap_vec3(vector: *const leap_sys::LEAP_VECTOR) -> UltraleapVec3 {
    if vector.is_null() {
        return UltraleapVec3::ZERO;
    }

    let vector = unsafe { ptr::read_unaligned(vector) };
    let [x, y, z] = unsafe { ptr::read_unaligned(ptr::addr_of!(vector.__bindgen_anon_1.v)) };
    UltraleapVec3::new(x as f64, y as f64, z as f64)
}

fn leap_vec3_meters(vector: *const leap_sys::LEAP_VECTOR) -> UltraleapVec3 {
    let vector = leap_vec3(vector);
    UltraleapVec3::new(
        millimeters_to_meters(vector.x),
        millimeters_to_meters(vector.y),
        millimeters_to_meters(vector.z),
    )
}

fn millimeters_to_meters(value: f64) -> f64 {
    value / MILLIMETERS_PER_METER
}
