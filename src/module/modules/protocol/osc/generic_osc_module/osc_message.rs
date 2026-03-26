use golden_core::parameter::ParamValue;
use rosc::{OscMessage, OscPacket, OscType};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OscDecodedMessage {
    pub address: String,
    pub payload: OscValuePayload,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OscValuePayload {
    Single(ParamValue),
    Multi(Vec<ParamValue>),
}

pub(crate) fn encode_packet(address: &str, payload: &OscValuePayload) -> Result<OscPacket, String> {
    if address.trim().is_empty() {
        return Err("OSC address cannot be empty".to_string());
    }

    Ok(OscPacket::Message(OscMessage {
        addr: address.to_string(),
        args: encode_payload(payload)?,
    }))
}

pub(crate) fn decode_packet_messages(packet: OscPacket) -> Vec<Result<OscDecodedMessage, String>> {
    let mut decoded = Vec::new();
    flatten_packet(packet, &mut decoded);
    decoded
}

fn flatten_packet(packet: OscPacket, decoded: &mut Vec<Result<OscDecodedMessage, String>>) {
    match packet {
        OscPacket::Message(message) => decoded.push(decode_message(message)),
        OscPacket::Bundle(bundle) => {
            for packet in bundle.content {
                flatten_packet(packet, decoded);
            }
        }
    }
}

fn decode_message(message: OscMessage) -> Result<OscDecodedMessage, String> {
    let payload = decode_args(message.args.as_slice());
    Ok(OscDecodedMessage {
        address: message.addr,
        payload,
    })
}

fn encode_payload(payload: &OscValuePayload) -> Result<Vec<OscType>, String> {
    match payload {
        OscValuePayload::Single(value) => encode_param_value(value),
        OscValuePayload::Multi(values) => values.iter().map(encode_single_argument).collect(),
    }
}

fn encode_param_value(value: &ParamValue) -> Result<Vec<OscType>, String> {
    let args = match value {
        ParamValue::Trigger() => Vec::new(),
        ParamValue::Int(value) => vec![OscType::Int(*value)],
        ParamValue::Float(value) => vec![OscType::Float(*value as f32)],
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            vec![OscType::String(value.clone())]
        }
        ParamValue::Bool(value) => vec![OscType::Bool(*value)],
        ParamValue::Vec2(x, y) => vec![OscType::Float(*x as f32), OscType::Float(*y as f32)],
        ParamValue::Vec3(x, y, z) => vec![
            OscType::Float(*x as f32),
            OscType::Float(*y as f32),
            OscType::Float(*z as f32),
        ],
        ParamValue::Color(r, g, b, a) => vec![
            OscType::Float(*r as f32),
            OscType::Float(*g as f32),
            OscType::Float(*b as f32),
            OscType::Float(*a as f32),
        ],
        ParamValue::CssValue(_) => {
            return Err("CSS values are not supported by the generic OSC module".to_string());
        }
        ParamValue::Reference(_) => {
            return Err("Node references are not supported by the generic OSC module".to_string());
        }
    };

    Ok(args)
}

fn encode_single_argument(value: &ParamValue) -> Result<OscType, String> {
    match value {
        ParamValue::Trigger() => Err("Trigger values cannot be encoded as one OSC argument".to_string()),
        ParamValue::Int(value) => Ok(OscType::Int(*value)),
        ParamValue::Float(value) => Ok(OscType::Float(*value as f32)),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            Ok(OscType::String(value.clone()))
        }
        ParamValue::Bool(value) => Ok(OscType::Bool(*value)),
        ParamValue::Color(r, g, b, a) => Ok(OscType::Color(rosc::OscColor {
            red: color_channel_to_u8(*r),
            green: color_channel_to_u8(*g),
            blue: color_channel_to_u8(*b),
            alpha: color_channel_to_u8(*a),
        })),
        ParamValue::Vec2(_, _) | ParamValue::Vec3(_, _, _) | ParamValue::CssValue(_) | ParamValue::Reference(_) => {
            Err("Only scalar-like values are supported inside OSC multi-value folders".to_string())
        }
    }
}

fn decode_args(args: &[OscType]) -> OscValuePayload {
    match try_decode_supported_cast(args) {
        Some(value) => OscValuePayload::Single(value),
        None => OscValuePayload::Multi(args.iter().map(decode_single_argument_lossy).collect()),
    }
}

fn try_decode_supported_cast(args: &[OscType]) -> Option<ParamValue> {
    match args {
        [] => Some(ParamValue::Trigger()),
        [value] => decode_supported_single(value),
        [OscType::Float(first), OscType::Float(second)] => Some(ParamValue::Vec2(
            f64::from(*first),
            f64::from(*second),
        )),
        [OscType::Float(first), OscType::Float(second), OscType::Float(third)] => Some(ParamValue::Vec3(
            f64::from(*first),
            f64::from(*second),
            f64::from(*third),
        )),
        [OscType::Float(r), OscType::Float(g), OscType::Float(b), OscType::Float(a)] => Some(ParamValue::Color(
            f64::from(*r),
            f64::from(*g),
            f64::from(*b),
            f64::from(*a),
        )),
        [OscType::Int(r), OscType::Int(g), OscType::Int(b), OscType::Int(a)] => Some(ParamValue::Color(
            f64::from(*r) / 255.0,
            f64::from(*g) / 255.0,
            f64::from(*b) / 255.0,
            f64::from(*a) / 255.0,
        )),
        _ => None,
    }
}

fn decode_supported_single(value: &OscType) -> Option<ParamValue> {
    match value {
        OscType::Int(value) => Some(ParamValue::Int(*value)),
        OscType::Long(value) => i32::try_from(*value).ok().map(ParamValue::Int),
        OscType::Float(value) => Some(ParamValue::Float((*value).into())),
        OscType::Double(value) => Some(ParamValue::Float(*value)),
        OscType::String(value) => Some(ParamValue::Str(value.clone())),
        OscType::Char(value) => Some(ParamValue::Str(value.to_string())),
        OscType::Bool(value) => Some(ParamValue::Bool(*value)),
        OscType::Color(value) => Some(ParamValue::Color(
            f64::from(value.red) / 255.0,
            f64::from(value.green) / 255.0,
            f64::from(value.blue) / 255.0,
            f64::from(value.alpha) / 255.0,
        )),
        _ => None,
    }
}

fn decode_single_argument_lossy(value: &OscType) -> ParamValue {
    if let Some(decoded) = decode_supported_single(value) {
        return decoded;
    }

    match value {
        OscType::Midi(message) => ParamValue::Str(format!(
            "midi({}, {}, {}, {})",
            message.port, message.status, message.data1, message.data2
        )),
        OscType::Nil => ParamValue::Str("nil".to_string()),
        OscType::Inf => ParamValue::Str("inf".to_string()),
        OscType::Blob(bytes) => ParamValue::Str(format!("blob[{}]", bytes.len())),
        OscType::Array(values) => ParamValue::Str(format!("array[{}]", values.content.len())),
        other => ParamValue::Str(format!("{other:?}")),
    }
}

fn color_channel_to_u8(value: f64) -> u8 {
    value.clamp(0.0, 1.0).mul_add(255.0, 0.0).round() as u8
}
