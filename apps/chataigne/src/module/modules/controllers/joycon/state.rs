use super::*;

#[derive(Clone, Copy)]
pub(super) enum JoyConBoolParam {
    Activity,
    IsConnected,
    LeftControllerConnected,
    RightControllerConnected,
}

#[derive(Clone, Copy)]
pub(super) enum JoyConButtonParam {
    LeftDown,
    LeftUp,
    LeftRight,
    LeftLeft,
    LeftSL,
    LeftSR,
    LeftL,
    LeftZL,
    LeftMinus,
    LeftStickButton,
    LeftCapture,
    RightY,
    RightX,
    RightB,
    RightA,
    RightSR,
    RightSL,
    RightR,
    RightZR,
    RightPlus,
    RightStickButton,
    RightHome,
}

#[derive(Clone, Copy)]
pub(super) enum JoyConVec2Param {
    LeftStick,
    RightStick,
}

#[derive(Clone, Copy)]
pub(super) enum JoyConMotionParam {
    LeftOrientation,
    LeftAccelerometer,
    LeftGyroscope,
    RightOrientation,
    RightAccelerometer,
    RightGyroscope,
}

#[derive(Clone, Copy)]
pub(super) struct JoyConMotionTargets {
    pub(super) orientation: JoyConMotionParam,
    pub(super) accelerometer: JoyConMotionParam,
    pub(super) gyroscope: JoyConMotionParam,
}

#[derive(Clone, Copy)]
pub(super) struct JoyConMotionSample {
    pub(super) pitch: f64,
    pub(super) roll: f64,
    pub(super) accelerometer: (f64, f64, f64),
    pub(super) gyroscope: (f64, f64, f64),
}

impl From<&runtime::JoyConControllerStateSnapshot> for JoyConMotionSample {
    fn from(state: &runtime::JoyConControllerStateSnapshot) -> Self {
        Self {
            pitch: state.orientation_pitch,
            roll: state.orientation_roll,
            accelerometer: state.accelerometer,
            gyroscope: state.gyroscope,
        }
    }
}

const JOYCON_LEFT_BUTTON_PARAMS: &[JoyConButtonParam] = &[
    JoyConButtonParam::LeftDown,
    JoyConButtonParam::LeftUp,
    JoyConButtonParam::LeftRight,
    JoyConButtonParam::LeftLeft,
    JoyConButtonParam::LeftSL,
    JoyConButtonParam::LeftSR,
    JoyConButtonParam::LeftL,
    JoyConButtonParam::LeftZL,
    JoyConButtonParam::LeftMinus,
    JoyConButtonParam::LeftStickButton,
    JoyConButtonParam::LeftCapture,
];

const JOYCON_RIGHT_BUTTON_PARAMS: &[JoyConButtonParam] = &[
    JoyConButtonParam::RightY,
    JoyConButtonParam::RightX,
    JoyConButtonParam::RightB,
    JoyConButtonParam::RightA,
    JoyConButtonParam::RightSR,
    JoyConButtonParam::RightSL,
    JoyConButtonParam::RightR,
    JoyConButtonParam::RightZR,
    JoyConButtonParam::RightPlus,
    JoyConButtonParam::RightStickButton,
    JoyConButtonParam::RightHome,
];

pub(super) fn float_changed(current: f64, next: f64) -> bool {
    (current - next).abs() > f64::EPSILON
}

pub(super) fn joycon_side_id(side: JoyConSide) -> &'static str {
    match side {
        JoyConSide::Left => "left",
        JoyConSide::Right => "right",
    }
}

pub(super) fn joycon_side_button_params(side: JoyConSide) -> &'static [JoyConButtonParam] {
    match side {
        JoyConSide::Left => JOYCON_LEFT_BUTTON_PARAMS,
        JoyConSide::Right => JOYCON_RIGHT_BUTTON_PARAMS,
    }
}

pub(super) fn joycon_button_id(button: JoyConButtonParam) -> &'static str {
    match button {
        JoyConButtonParam::LeftDown => "down",
        JoyConButtonParam::LeftUp => "up",
        JoyConButtonParam::LeftRight => "right",
        JoyConButtonParam::LeftLeft => "left",
        JoyConButtonParam::LeftSL => "sl",
        JoyConButtonParam::LeftSR => "sr",
        JoyConButtonParam::LeftL => "l",
        JoyConButtonParam::LeftZL => "zl",
        JoyConButtonParam::LeftMinus => "minus",
        JoyConButtonParam::LeftStickButton => "stick",
        JoyConButtonParam::LeftCapture => "capture",
        JoyConButtonParam::RightY => "y",
        JoyConButtonParam::RightX => "x",
        JoyConButtonParam::RightB => "b",
        JoyConButtonParam::RightA => "a",
        JoyConButtonParam::RightSR => "sr",
        JoyConButtonParam::RightSL => "sl",
        JoyConButtonParam::RightR => "r",
        JoyConButtonParam::RightZR => "zr",
        JoyConButtonParam::RightPlus => "plus",
        JoyConButtonParam::RightStickButton => "stick",
        JoyConButtonParam::RightHome => "home",
    }
}

pub(super) fn joycon_button_state(state: &runtime::JoyConControllerStateSnapshot, button: JoyConButtonParam) -> bool {
    match button {
        JoyConButtonParam::LeftDown => state.left_buttons.contains(&Buttons::Down),
        JoyConButtonParam::LeftUp => state.left_buttons.contains(&Buttons::Up),
        JoyConButtonParam::LeftRight => state.left_buttons.contains(&Buttons::Right),
        JoyConButtonParam::LeftLeft => state.left_buttons.contains(&Buttons::Left),
        JoyConButtonParam::LeftSL => state.left_buttons.contains(&Buttons::SL),
        JoyConButtonParam::LeftSR => state.left_buttons.contains(&Buttons::SR),
        JoyConButtonParam::LeftL => state.left_buttons.contains(&Buttons::L),
        JoyConButtonParam::LeftZL => state.left_buttons.contains(&Buttons::ZL),
        JoyConButtonParam::LeftMinus => state.shared_buttons.contains(&Buttons::Minus),
        JoyConButtonParam::LeftStickButton => state.shared_buttons.contains(&Buttons::LStick),
        JoyConButtonParam::LeftCapture => state.shared_buttons.contains(&Buttons::Capture),
        JoyConButtonParam::RightY => state.right_buttons.contains(&Buttons::Y),
        JoyConButtonParam::RightX => state.right_buttons.contains(&Buttons::X),
        JoyConButtonParam::RightB => state.right_buttons.contains(&Buttons::B),
        JoyConButtonParam::RightA => state.right_buttons.contains(&Buttons::A),
        JoyConButtonParam::RightSR => state.right_buttons.contains(&Buttons::SR),
        JoyConButtonParam::RightSL => state.right_buttons.contains(&Buttons::SL),
        JoyConButtonParam::RightR => state.right_buttons.contains(&Buttons::R),
        JoyConButtonParam::RightZR => state.right_buttons.contains(&Buttons::ZR),
        JoyConButtonParam::RightPlus => state.shared_buttons.contains(&Buttons::Plus),
        JoyConButtonParam::RightStickButton => state.shared_buttons.contains(&Buttons::RStick),
        JoyConButtonParam::RightHome => state.shared_buttons.contains(&Buttons::Home),
    }
}

pub(super) fn joycon_buttons_arg(
    side: JoyConSide,
    state: &runtime::JoyConControllerStateSnapshot,
) -> serde_json::Value {
    let mut buttons = serde_json::Map::new();
    for button in joycon_side_button_params(side) {
        buttons.insert(
            joycon_button_id(*button).to_string(),
            serde_json::Value::Bool(joycon_button_state(state, *button)),
        );
    }
    serde_json::Value::Object(buttons)
}

pub(super) fn controller_motion_arg(
    state: &runtime::JoyConControllerStateSnapshot,
    motion_mode: JoyConMotionDataMode,
) -> serde_json::Value {
    let mode = match motion_mode {
        JoyConMotionDataMode::None => JOYCON_MOTION_DATA_NONE,
        JoyConMotionDataMode::Orientation => JOYCON_MOTION_DATA_ORIENTATION,
        JoyConMotionDataMode::All => JOYCON_MOTION_DATA_ALL,
    };

    let orientation = match motion_mode {
        JoyConMotionDataMode::None => serde_json::Value::Null,
        JoyConMotionDataMode::Orientation | JoyConMotionDataMode::All => serde_json::json!({
            "pitch": state.orientation_pitch,
            "roll": state.orientation_roll,
        }),
    };
    let accelerometer = match motion_mode {
        JoyConMotionDataMode::All => serde_json::json!({
            "x": state.accelerometer.0,
            "y": state.accelerometer.1,
            "z": state.accelerometer.2,
        }),
        JoyConMotionDataMode::None | JoyConMotionDataMode::Orientation => serde_json::Value::Null,
    };
    let gyroscope = match motion_mode {
        JoyConMotionDataMode::All => serde_json::json!({
            "x": state.gyroscope.0,
            "y": state.gyroscope.1,
            "z": state.gyroscope.2,
        }),
        JoyConMotionDataMode::None | JoyConMotionDataMode::Orientation => serde_json::Value::Null,
    };

    serde_json::json!({
        "mode": mode,
        "orientation": orientation,
        "accelerometer": accelerometer,
        "gyroscope": gyroscope,
    })
}

pub(super) fn process_stick_axis_value(raw_value: f64, dead_zone: f64) -> f64 {
    let value = raw_value.clamp(-1.0, 1.0);
    let dead_zone = dead_zone.clamp(0.0, 1.0);
    if dead_zone >= 1.0 || value.abs() <= dead_zone {
        return 0.0;
    }

    let scaled = (value.abs() - dead_zone) / (1.0 - dead_zone);
    scaled.copysign(value).clamp(-1.0, 1.0)
}

pub(super) fn processed_stick_values(x: f64, y: f64, dead_zone: f64) -> (f64, f64) {
    (
        process_stick_axis_value(x, dead_zone),
        process_stick_axis_value(y, dead_zone),
    )
}

pub(super) fn runtime_state_has_visible_input_change(
    previous: &JoyConRuntimeState,
    next: &JoyConRuntimeState,
    dead_zone: f64,
    motion_mode: JoyConMotionDataMode,
) -> bool {
    controller_has_visible_input_change(&previous.left, &next.left, dead_zone, motion_mode)
        || controller_has_visible_input_change(&previous.right, &next.right, dead_zone, motion_mode)
}

pub(super) fn controller_has_visible_input_change(
    previous: &runtime::JoyConControllerStateSnapshot,
    next: &runtime::JoyConControllerStateSnapshot,
    dead_zone: f64,
    motion_mode: JoyConMotionDataMode,
) -> bool {
    controller_buttons_changed(previous, next)
        || controller_stick_changed(previous, next, dead_zone)
        || controller_motion_changed(previous, next, motion_mode)
}

pub(super) fn controller_buttons_changed(
    previous: &runtime::JoyConControllerStateSnapshot,
    next: &runtime::JoyConControllerStateSnapshot,
) -> bool {
    button_list_changed(&previous.left_buttons, &next.left_buttons)
        || button_list_changed(&previous.right_buttons, &next.right_buttons)
        || button_list_changed(&previous.shared_buttons, &next.shared_buttons)
}

pub(super) fn button_list_changed(previous: &[Buttons], next: &[Buttons]) -> bool {
    previous.len() != next.len() || previous.iter().any(|button| !next.contains(button))
}

pub(super) fn controller_stick_changed(
    previous: &runtime::JoyConControllerStateSnapshot,
    next: &runtime::JoyConControllerStateSnapshot,
    dead_zone: f64,
) -> bool {
    let (previous_x, previous_y) = processed_stick_values(previous.stick_x, previous.stick_y, dead_zone);
    let (next_x, next_y) = processed_stick_values(next.stick_x, next.stick_y, dead_zone);
    float_changed(previous_x, next_x) || float_changed(previous_y, next_y)
}

pub(super) fn controller_motion_changed(
    previous: &runtime::JoyConControllerStateSnapshot,
    next: &runtime::JoyConControllerStateSnapshot,
    motion_mode: JoyConMotionDataMode,
) -> bool {
    match motion_mode {
        JoyConMotionDataMode::None => false,
        JoyConMotionDataMode::Orientation => {
            float_changed(previous.orientation_pitch, next.orientation_pitch)
                || float_changed(previous.orientation_roll, next.orientation_roll)
        }
        JoyConMotionDataMode::All => {
            float_changed(previous.orientation_pitch, next.orientation_pitch)
                || float_changed(previous.orientation_roll, next.orientation_roll)
                || float_changed(previous.accelerometer.0, next.accelerometer.0)
                || float_changed(previous.accelerometer.1, next.accelerometer.1)
                || float_changed(previous.accelerometer.2, next.accelerometer.2)
                || float_changed(previous.gyroscope.0, next.gyroscope.0)
                || float_changed(previous.gyroscope.1, next.gyroscope.1)
                || float_changed(previous.gyroscope.2, next.gyroscope.2)
        }
    }
}

pub(super) fn processing_interval_from_fps_cap(fps_cap: i32) -> Duration {
    let clamped = fps_cap.clamp(1, JOYCON_MODULE_UPDATE_RATE_HZ as i32);
    Duration::from_secs_f64(1.0 / f64::from(clamped))
}

pub(super) fn connection_state_changed(previous: &JoyConRuntimeState, next: &JoyConRuntimeState) -> bool {
    previous.left.connected != next.left.connected || previous.right.connected != next.right.connected
}

pub(super) fn runtime_expected_to_be_live(
    pending_state: Option<&JoyConRuntimeState>,
    last_state: &JoyConRuntimeState,
) -> bool {
    pending_state.is_some_and(JoyConRuntimeState::any_connected) || last_state.any_connected()
}

pub(super) fn summarize_runtime_state_for_log(
    state: &JoyConRuntimeState,
    dead_zone: f64,
    motion_mode: JoyConMotionDataMode,
) -> String {
    let mut parts = Vec::new();
    if state.left.connected {
        let (stick_x, stick_y) = processed_stick_values(state.left.stick_x, state.left.stick_y, dead_zone);
        let motion_suffix = if motion_mode == JoyConMotionDataMode::None {
            String::new()
        } else {
            " motion=on".to_string()
        };
        parts.push(format!(
            "L stick=({}, {}) buttons={}{}",
            format_axis_value(stick_x),
            format_axis_value(stick_y),
            joycon_side_button_params(JoyConSide::Left)
                .iter()
                .filter(|button| joycon_button_state(&state.left, **button))
                .count(),
            motion_suffix,
        ));
    }
    if state.right.connected {
        let (stick_x, stick_y) = processed_stick_values(state.right.stick_x, state.right.stick_y, dead_zone);
        let motion_suffix = if motion_mode == JoyConMotionDataMode::None {
            String::new()
        } else {
            " motion=on".to_string()
        };
        parts.push(format!(
            "R stick=({}, {}) buttons={}{}",
            format_axis_value(stick_x),
            format_axis_value(stick_y),
            joycon_side_button_params(JoyConSide::Right)
                .iter()
                .filter(|button| joycon_button_state(&state.right, **button))
                .count(),
            motion_suffix,
        ));
    }

    if parts.is_empty() {
        "Joy-Con input idle".to_string()
    } else {
        format!("Joy-Con input: {}", parts.join(" | "))
    }
}

pub(super) fn format_axis_value(value: f64) -> String {
    format!("{value:.3}")
}

pub(super) fn script_vibrate_request(args: &[ParamValue]) -> Option<Result<JoyConVibrateRequest, String>> {
    let frequency_hz = match args.first() {
        Some(value) => match param_value_as_f32(value) {
            Some(value) => value,
            None => {
                return Some(Err(
                    "method 'vibrate' expects a numeric frequencyHz argument".to_string()
                ))
            }
        },
        None => 300.0,
    };
    if !frequency_hz.is_finite() || !(0.0..=1252.0).contains(&frequency_hz) {
        return Some(Err(
            "method 'vibrate' expects frequencyHz between 0.0 and 1252.0".to_string()
        ));
    }

    let amplitude = match args.get(1) {
        Some(value) => match param_value_as_f32(value) {
            Some(value) => value,
            None => return Some(Err("method 'vibrate' expects a numeric amplitude argument".to_string())),
        },
        None => 0.9,
    };
    if !amplitude.is_finite() || !(0.0..=1.799).contains(&amplitude) {
        return Some(Err(
            "method 'vibrate' expects amplitude between 0.0 and 1.799".to_string()
        ));
    }

    let duration_ms = match args.get(2) {
        Some(value) => match param_value_as_u32(value) {
            Some(value) if value > 0 => value,
            Some(_) => return Some(Err("method 'vibrate' expects durationMs greater than 0".to_string())),
            None => return Some(Err("method 'vibrate' expects a numeric durationMs argument".to_string())),
        },
        None => 60,
    };

    let target = match args.get(3) {
        Some(value) => match value.as_str() {
            Some(value) => match parse_joycon_target(&value) {
                Ok(value) => value.to_string(),
                Err(error) => return Some(Err(format!("method 'vibrate' {error}"))),
            },
            None => return Some(Err("method 'vibrate' expects a string target argument".to_string())),
        },
        None => JOYCON_TARGET_BOTH.to_string(),
    };

    Some(Ok(JoyConVibrateRequest {
        target,
        frequency_hz,
        amplitude,
        duration_ms,
        description: "vibrate".to_string(),
    }))
}

pub(super) fn script_set_player_lights_request(args: &[ParamValue]) -> Option<Result<JoyConSetLedRequest, String>> {
    let led_1 = match script_led_state_arg("setPlayerLights", args, 0) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let led_2 = match script_led_state_arg("setPlayerLights", args, 1) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let led_3 = match script_led_state_arg("setPlayerLights", args, 2) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let led_4 = match script_led_state_arg("setPlayerLights", args, 3) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let target = match args.get(4) {
        Some(value) => match value.as_str() {
            Some(value) => match parse_joycon_target(&value) {
                Ok(value) => value.to_string(),
                Err(error) => return Some(Err(format!("method 'setPlayerLights' {error}"))),
            },
            None => {
                return Some(Err(
                    "method 'setPlayerLights' expects a string target argument".to_string()
                ))
            }
        },
        None => JOYCON_TARGET_BOTH.to_string(),
    };

    Some(Ok(JoyConSetLedRequest {
        target,
        led_1,
        led_2,
        led_3,
        led_4,
        description: "set player lights".to_string(),
    }))
}

pub(super) fn script_led_state_arg(method: &str, args: &[ParamValue], index: usize) -> Result<String, String> {
    let Some(value) = args.get(index) else {
        return Ok(JOYCON_LED_STATE_OFF.to_string());
    };
    let Some(value) = value.as_str() else {
        return Err(format!("method '{method}' expects LED arguments to be strings"));
    };

    parse_joycon_led_state(&value)
        .map(str::to_string)
        .map_err(|error| format!("method '{method}' {error}"))
}

pub(super) fn parse_joycon_target(value: &str) -> Result<&str, String> {
    JoyConControllerTarget::from_variant_id(value)
        .map(JoyConControllerTarget::variant_id)
        .ok_or_else(|| format!("expects target to be 'left', 'right', or 'both', got '{value}'"))
}

pub(super) fn parse_joycon_led_state(value: &str) -> Result<&str, String> {
    match value.trim() {
        JOYCON_LED_STATE_OFF => Ok(JOYCON_LED_STATE_OFF),
        JOYCON_LED_STATE_ON => Ok(JOYCON_LED_STATE_ON),
        JOYCON_LED_STATE_FLASH => Ok(JOYCON_LED_STATE_FLASH),
        _ => Err(format!("expects LED states 'off', 'on', or 'flash', got '{value}'")),
    }
}

pub(super) fn param_value_as_number(value: &ParamValue) -> Option<f64> {
    value.as_float().or_else(|| value.as_int().map(f64::from))
}

pub(super) fn param_value_as_f32(value: &ParamValue) -> Option<f32> {
    let number = param_value_as_number(value)?;
    number.is_finite().then_some(number as f32)
}

pub(super) fn param_value_as_u32(value: &ParamValue) -> Option<u32> {
    let number = param_value_as_number(value)?;
    if !number.is_finite() {
        return None;
    }

    let rounded = number.round();
    (rounded >= 0.0 && rounded <= f64::from(u32::MAX)).then_some(rounded as u32)
}
