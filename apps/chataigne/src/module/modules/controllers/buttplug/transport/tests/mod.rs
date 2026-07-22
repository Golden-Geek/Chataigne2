use super::{normalize_path, output_command};
use crate::app::module::common::buttplug::{
    ButtplugSetOutputRequest, BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION, BUTTPLUG_OUTPUT_VIBRATE,
};

#[test]
fn normalize_path_adds_leading_slash() {
    assert_eq!(normalize_path(""), "/");
    assert_eq!(normalize_path("buttplug"), "/buttplug");
    assert_eq!(normalize_path("/"), "/");
}

#[test]
fn output_command_rejects_values_outside_normalized_range() {
    let request = ButtplugSetOutputRequest {
        target: "all".to_string(),
        output: BUTTPLUG_OUTPUT_VIBRATE.to_string(),
        value: 2.0,
        duration_ms: 1000,
        description: "test".to_string(),
    };

    assert!(output_command(&request).is_err());
}

#[test]
fn position_with_duration_accepts_positive_duration() {
    let request = ButtplugSetOutputRequest {
        target: "all".to_string(),
        output: BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION.to_string(),
        value: 0.5,
        duration_ms: 0,
        description: "test".to_string(),
    };

    assert!(output_command(&request).is_ok());
}
