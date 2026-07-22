use std::time::Duration;

use super::{
    low_pass_vec2, low_pass_vec3, manager_scan_interval_for_connected_slots, report_is_stale, should_attempt_attach,
    JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL, JOYCON_MANAGER_FULL_SCAN_INTERVAL, JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL,
    JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT,
};

#[test]
fn joycon_manager_scans_fastest_when_no_slots_are_attached() {
    assert_eq!(
        manager_scan_interval_for_connected_slots(0),
        JOYCON_MANAGER_DISCOVERY_SCAN_INTERVAL,
    );
}

#[test]
fn joycon_manager_slows_background_scanning_when_streaming() {
    assert_eq!(
        manager_scan_interval_for_connected_slots(1),
        JOYCON_MANAGER_PARTIAL_SCAN_INTERVAL,
    );
    assert_eq!(
        manager_scan_interval_for_connected_slots(2),
        JOYCON_MANAGER_FULL_SCAN_INTERVAL,
    );
}

#[test]
fn joycon_attach_attempts_stop_once_all_supported_slots_are_filled() {
    assert!(should_attempt_attach(0));
    assert!(should_attempt_attach(1));
    assert!(!should_attempt_attach(2));
}

#[test]
fn joycon_full_scan_interval_is_long_enough_to_leave_input_loop_alone() {
    assert!(JOYCON_MANAGER_FULL_SCAN_INTERVAL >= Duration::from_secs(5));
}

#[test]
fn joycon_report_heartbeat_waits_for_timeout() {
    let now = std::time::Instant::now();
    let just_before_timeout = now + JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT - Duration::from_millis(1);

    assert!(!report_is_stale(Some(now), just_before_timeout, JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT));
}

#[test]
fn joycon_report_heartbeat_marks_stale_after_timeout() {
    let now = std::time::Instant::now();
    let at_timeout = now + JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT;

    assert!(report_is_stale(Some(now), at_timeout, JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT));
    assert!(!report_is_stale(None, at_timeout, JOYCON_REPORT_STALE_DISCONNECT_TIMEOUT));
}

#[test]
fn joycon_orientation_filter_damps_single_frame_jumps() {
    assert_eq!(low_pass_vec2(None, (10.0, -10.0), 0.3), (10.0, -10.0));
    assert_eq!(low_pass_vec2(Some((0.0, 0.0)), (30.0, -30.0), 0.3), (9.0, -9.0));
}

#[test]
fn joycon_accelerometer_filter_damps_single_frame_jumps() {
    assert_eq!(low_pass_vec3(None, (100.0, 200.0, 300.0), 0.2), (100.0, 200.0, 300.0));
    assert_eq!(
        low_pass_vec3(Some((0.0, 0.0, 0.0)), (100.0, 200.0, 300.0), 0.2),
        (20.0, 40.0, 60.0),
    );
}
