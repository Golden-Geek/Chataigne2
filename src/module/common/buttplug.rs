use golden_core::parameter::{ParamValue, ParameterEnumOption};
use serde::{Deserialize, Serialize};

pub(crate) const BUTTPLUG_SET_OUTPUT_COMMAND_NODE_TYPE: &str = "buttplug_set_output_command";
pub(crate) const BUTTPLUG_STOP_DEVICE_COMMAND_NODE_TYPE: &str = "buttplug_stop_device_command";
pub(crate) const BUTTPLUG_STOP_ALL_DEVICES_COMMAND_NODE_TYPE: &str = "buttplug_stop_all_devices_command";
pub(crate) const BUTTPLUG_START_SCANNING_COMMAND_NODE_TYPE: &str = "buttplug_start_scanning_command";
pub(crate) const BUTTPLUG_STOP_SCANNING_COMMAND_NODE_TYPE: &str = "buttplug_stop_scanning_command";

pub(crate) const BUTTPLUG_TARGET_SELECTED: &str = "selected";
pub(crate) const BUTTPLUG_TARGET_ALL: &str = "all";
pub(crate) const BUTTPLUG_TARGET_NONE: &str = "none";
pub(crate) const BUTTPLUG_DEVICE_VARIANT_PREFIX: &str = "device:";

pub(crate) const BUTTPLUG_OUTPUT_VIBRATE: &str = "vibrate";
pub(crate) const BUTTPLUG_OUTPUT_ROTATE: &str = "rotate";
pub(crate) const BUTTPLUG_OUTPUT_OSCILLATE: &str = "oscillate";
pub(crate) const BUTTPLUG_OUTPUT_CONSTRICT: &str = "constrict";
pub(crate) const BUTTPLUG_OUTPUT_POSITION: &str = "position";
pub(crate) const BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION: &str = "hw_position_with_duration";
pub(crate) const BUTTPLUG_OUTPUT_SPRAY: &str = "spray";
pub(crate) const BUTTPLUG_OUTPUT_LED: &str = "led";
pub(crate) const BUTTPLUG_OUTPUT_TEMPERATURE: &str = "temperature";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ButtplugSetOutputRequest {
    pub target: String,
    pub output: String,
    pub value: f64,
    pub duration_ms: u32,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ButtplugTargetRequest {
    pub target: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct ButtplugControlRequest {
    pub description: String,
}

pub(crate) fn buttplug_output_enum_options() -> Vec<ParameterEnumOption> {
    [
        (BUTTPLUG_OUTPUT_VIBRATE, "Vibrate"),
        (BUTTPLUG_OUTPUT_ROTATE, "Rotate"),
        (BUTTPLUG_OUTPUT_OSCILLATE, "Oscillate"),
        (BUTTPLUG_OUTPUT_CONSTRICT, "Constrict"),
        (BUTTPLUG_OUTPUT_POSITION, "Position"),
        (
            BUTTPLUG_OUTPUT_HW_POSITION_WITH_DURATION,
            "Position With Duration",
        ),
        (BUTTPLUG_OUTPUT_SPRAY, "Spray"),
        (BUTTPLUG_OUTPUT_LED, "LED"),
        (BUTTPLUG_OUTPUT_TEMPERATURE, "Temperature"),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordering, (variant_id, label))| ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering as i32),
    })
    .collect()
}
