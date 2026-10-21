use super::*;

pub(super) fn midi_message_script_payload(message: &MidiMessage) -> serde_json::Value {
    match message {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity,
        } => serde_json::json!({
            "type": "noteOff",
            "channel": channel,
            "note": note,
            "velocity": velocity,
        }),
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        } => serde_json::json!({
            "type": "noteOn",
            "channel": channel,
            "note": note,
            "velocity": velocity,
        }),
        MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        } => serde_json::json!({
            "type": "polyPressure",
            "channel": channel,
            "note": note,
            "pressure": pressure,
        }),
        MidiMessage::ControlChange {
            channel,
            controller,
            value,
        } => serde_json::json!({
            "type": "controlChange",
            "channel": channel,
            "controller": controller,
            "value": value,
        }),
        MidiMessage::ProgramChange { channel, program } => serde_json::json!({
            "type": "programChange",
            "channel": channel,
            "program": program,
        }),
        MidiMessage::ChannelPressure { channel, pressure } => serde_json::json!({
            "type": "channelPressure",
            "channel": channel,
            "pressure": pressure,
        }),
        MidiMessage::PitchBend { channel, value } => serde_json::json!({
            "type": "pitchBend",
            "channel": channel,
            "value": value,
        }),
        MidiMessage::System(system) => midi_system_message_script_payload(system),
    }
}

pub(super) fn midi_system_message_script_payload(message: &MidiSystemMessage) -> serde_json::Value {
    match message {
        MidiSystemMessage::TimeCodeQuarterFrame { value } => serde_json::json!({
            "type": "timeCodeQuarterFrame",
            "value": value,
        }),
        MidiSystemMessage::SongPosition { position } => serde_json::json!({
            "type": "songPosition",
            "position": position,
        }),
        MidiSystemMessage::SongSelect { song } => serde_json::json!({
            "type": "songSelect",
            "song": song,
        }),
        MidiSystemMessage::TuneRequest => serde_json::json!({ "type": "tuneRequest" }),
        MidiSystemMessage::TimingClock => serde_json::json!({ "type": "timingClock" }),
        MidiSystemMessage::Start => serde_json::json!({ "type": "start" }),
        MidiSystemMessage::Continue => serde_json::json!({ "type": "continue" }),
        MidiSystemMessage::Stop => serde_json::json!({ "type": "stop" }),
        MidiSystemMessage::ActiveSensing => serde_json::json!({ "type": "activeSensing" }),
        MidiSystemMessage::Reset => serde_json::json!({ "type": "reset" }),
        MidiSystemMessage::Sysex { bytes } => serde_json::json!({
            "type": "sysEx",
            "bytes": crate::app::module::script_api::bytes_arg(bytes.as_slice()),
        }),
    }
}

pub(super) fn script_channel_arg(args: &[ParamValue], index: usize, _name: &str) -> u8 {
    clamp_channel_i32(script_i32_arg_or(args, index, DEFAULT_SCRIPT_MIDI_CHANNEL))
}

pub(super) fn script_u7_arg(args: &[ParamValue], index: usize, _name: &str) -> u8 {
    clamp_i32_to_u7(script_i32_arg_or(args, index, 0))
}

pub(super) fn script_u7_arg_or(args: &[ParamValue], index: usize, fallback: i32) -> u8 {
    clamp_i32_to_u7(script_i32_arg_or(args, index, fallback))
}

pub(super) fn script_u14_arg(args: &[ParamValue], index: usize, _name: &str) -> u16 {
    clamp_i32_to_u14(script_i32_arg_or(args, index, i32::from(MIDI_PITCH_BEND_CENTER)))
}

pub(super) fn script_nonnegative_u64_arg_or(args: &[ParamValue], index: usize, fallback: u64) -> u64 {
    u64::try_from(script_i32_arg_or(args, index, i32::try_from(fallback).unwrap_or(i32::MAX)).max(0))
        .unwrap_or(fallback)
}

pub(super) fn script_i32_arg_or(args: &[ParamValue], index: usize, fallback: i32) -> i32 {
    let Some(value) = args.get(index) else {
        return fallback;
    };
    value
        .as_int()
        .or_else(|| value.as_float().map(|value| value.round() as i32))
        .unwrap_or(fallback)
}

pub(super) fn script_bytes_from_args(args: &[ParamValue]) -> Result<Vec<u8>, String> {
    if args.len() == 1 {
        if let Some(text) = args[0].as_str() {
            return parse_script_byte_list(text.as_str());
        }
    }

    let mut bytes = Vec::new();
    for value in args {
        if let Some(text) = value.as_str() {
            bytes.extend(parse_script_byte_list(text.as_str())?);
            continue;
        }
        if let Some((x, y)) = value.as_vec2() {
            bytes.push(script_f64_byte(x)?);
            bytes.push(script_f64_byte(y)?);
            continue;
        }
        if let Some((x, y, z)) = value.as_vec3() {
            bytes.push(script_f64_byte(x)?);
            bytes.push(script_f64_byte(y)?);
            bytes.push(script_f64_byte(z)?);
            continue;
        }
        bytes.push(script_i32_byte(script_i32_arg_or(std::slice::from_ref(value), 0, -1))?);
    }
    Ok(bytes)
}

pub(super) fn parse_script_byte_list(text: &str) -> Result<Vec<u8>, String> {
    text.split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(parse_script_byte_token)
        .collect()
}

pub(super) fn parse_script_byte_token(token: &str) -> Result<u8, String> {
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|error| format!("invalid MIDI byte '{token}': {error}"));
    }

    if token
        .chars()
        .any(|character| matches!(character, 'a'..='f' | 'A'..='F'))
    {
        return u8::from_str_radix(token, 16).map_err(|error| format!("invalid MIDI byte '{token}': {error}"));
    }

    token
        .parse::<u8>()
        .map_err(|error| format!("invalid MIDI byte '{token}': {error}"))
}

pub(super) fn script_f64_byte(value: f64) -> Result<u8, String> {
    script_i32_byte(value.round() as i32)
}

pub(super) fn script_i32_byte(value: i32) -> Result<u8, String> {
    u8::try_from(value).map_err(|_| format!("MIDI byte {value} is outside the 0-255 range"))
}
