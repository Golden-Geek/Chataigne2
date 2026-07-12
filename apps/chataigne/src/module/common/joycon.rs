use golden_core::parameter::{ParamValue, ParameterEnumOption};
use serde::{Deserialize, Serialize};

pub(crate) const JOYCON_VIBRATE_COMMAND_NODE_TYPE: &str = "joycon_vibrate_command";
pub(crate) const JOYCON_SET_LED_COMMAND_NODE_TYPE: &str = "joycon_set_led_command";

pub(crate) const JOYCON_TARGET_LEFT: &str = "left";
pub(crate) const JOYCON_TARGET_RIGHT: &str = "right";
pub(crate) const JOYCON_TARGET_BOTH: &str = "both";

pub(crate) const JOYCON_MOTION_DATA_NONE: &str = "none";
pub(crate) const JOYCON_MOTION_DATA_ORIENTATION: &str = "orientation";
pub(crate) const JOYCON_MOTION_DATA_ALL: &str = "all";

pub(crate) const JOYCON_LED_STATE_OFF: &str = "off";
pub(crate) const JOYCON_LED_STATE_ON: &str = "on";
pub(crate) const JOYCON_LED_STATE_FLASH: &str = "flash";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JoyConControllerTarget {
    Left,
    Right,
    Both,
}

impl JoyConControllerTarget {
    pub(crate) fn from_variant_id(value: &str) -> Option<Self> {
        match value.trim() {
            JOYCON_TARGET_LEFT => Some(Self::Left),
            JOYCON_TARGET_RIGHT => Some(Self::Right),
            JOYCON_TARGET_BOTH => Some(Self::Both),
            _ => None,
        }
    }

    pub(crate) const fn variant_id(self) -> &'static str {
        match self {
            Self::Left => JOYCON_TARGET_LEFT,
            Self::Right => JOYCON_TARGET_RIGHT,
            Self::Both => JOYCON_TARGET_BOTH,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Left => "Left Controller",
            Self::Right => "Right Controller",
            Self::Both => "Both Controllers",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JoyConMotionDataMode {
    None,
    Orientation,
    All,
}

impl JoyConMotionDataMode {
    pub(crate) fn from_variant_id(value: &str) -> Option<Self> {
        match value.trim() {
            JOYCON_MOTION_DATA_NONE => Some(Self::None),
            JOYCON_MOTION_DATA_ORIENTATION => Some(Self::Orientation),
            JOYCON_MOTION_DATA_ALL => Some(Self::All),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct JoyConVibrateRequest {
    pub target: String,
    pub frequency_hz: f32,
    pub amplitude: f32,
    pub duration_ms: u32,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct JoyConSetLedRequest {
    pub target: String,
    pub led_1: String,
    pub led_2: String,
    pub led_3: String,
    pub led_4: String,
    pub description: String,
}

pub(crate) fn joycon_target_enum_options() -> Vec<ParameterEnumOption> {
    [
        JoyConControllerTarget::Left,
        JoyConControllerTarget::Right,
        JoyConControllerTarget::Both,
    ]
    .into_iter()
    .enumerate()
    .map(|(ordering, target)| enum_option(target.variant_id(), target.label(), ordering as i32))
    .collect()
}

pub(crate) fn joycon_motion_data_enum_options() -> Vec<ParameterEnumOption> {
    [
        (JOYCON_MOTION_DATA_NONE, "No Motion Data"),
        (JOYCON_MOTION_DATA_ORIENTATION, "Orientation"),
        (JOYCON_MOTION_DATA_ALL, "All (including Accelerometer)"),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordering, (variant_id, label))| enum_option(variant_id, label, ordering as i32))
    .collect()
}

pub(crate) fn joycon_led_state_enum_options() -> Vec<ParameterEnumOption> {
    [
        (JOYCON_LED_STATE_OFF, "Off"),
        (JOYCON_LED_STATE_ON, "On"),
        (JOYCON_LED_STATE_FLASH, "Flash"),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordering, (variant_id, label))| enum_option(variant_id, label, ordering as i32))
    .collect()
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}
