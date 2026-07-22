use golden_core::parameter::ParamValue;
use serde::{Deserialize, Serialize};

use super::parser::decode_text_escape_sequences;

pub(crate) const STREAMING_SEND_STRING_COMMAND_NODE_TYPE: &str = "streaming_send_string_command";
pub(crate) const STREAMING_SEND_BYTES_COMMAND_NODE_TYPE: &str = "streaming_send_bytes_command";
pub(crate) const STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE: &str = "streaming_send_hex_string_command";
pub(crate) const STREAMING_SEND_VALUES_COMMAND_NODE_TYPE: &str = "streaming_send_values_command";
pub(crate) const STREAMING_SEND_VALUES_AS_JSON_COMMAND_NODE_TYPE: &str = "streaming_send_values_as_json_command";
pub(crate) const STREAMING_COMMAND_NODE_TYPES: &[&str] = &[
    STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
    STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
    STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE,
    STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
    STREAMING_SEND_VALUES_AS_JSON_COMMAND_NODE_TYPE,
];

pub(crate) const LINE_ENDING_NONE: &str = "none";
pub(crate) const LINE_ENDING_NL: &str = "nl";
pub(crate) const LINE_ENDING_CR: &str = "cr";
pub(crate) const LINE_ENDING_CRLF: &str = "crlf";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StreamingSendFrameKind {
    Text,
    Binary,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct StreamingSendRequest {
    pub bytes: Vec<u8>,
    pub description: String,
    pub frame_kind: StreamingSendFrameKind,
}

pub(crate) fn string_request(text: &str, line_ending: &str) -> StreamingSendRequest {
    let mut output = decode_text_escape_sequences(text).into_bytes();
    output.extend_from_slice(line_ending_bytes(line_ending));
    StreamingSendRequest {
        bytes: output,
        description: "string".to_string(),
        frame_kind: StreamingSendFrameKind::Text,
    }
}

pub(crate) fn bytes_request(text: &str) -> Result<StreamingSendRequest, String> {
    Ok(StreamingSendRequest {
        bytes: parse_byte_list(text)?,
        description: "bytes".to_string(),
        frame_kind: StreamingSendFrameKind::Binary,
    })
}

pub(crate) fn hex_string_request(text: &str) -> Result<StreamingSendRequest, String> {
    Ok(StreamingSendRequest {
        bytes: parse_hex_string(text)?,
        description: "hex string".to_string(),
        frame_kind: StreamingSendFrameKind::Text,
    })
}

pub(crate) fn values_request(
    values: &[ParamValue],
    prefix: &str,
    suffix: &str,
    separator: &str,
) -> StreamingSendRequest {
    let body = values
        .iter()
        .map(value_to_streaming_string)
        .collect::<Vec<_>>()
        .join(decode_text_escape_sequences(separator).as_str());
    let text = format!(
        "{}{}{}",
        decode_text_escape_sequences(prefix),
        body,
        decode_text_escape_sequences(suffix)
    );

    StreamingSendRequest {
        bytes: text.into_bytes(),
        description: "values".to_string(),
        frame_kind: StreamingSendFrameKind::Binary,
    }
}

pub(crate) fn values_json_request(value: &serde_json::Value) -> Result<StreamingSendRequest, String> {
    let text = serde_json::to_string(value)
        .map_err(|error| format!("failed to encode values JSON payload: {error}"))?;

    Ok(StreamingSendRequest {
        bytes: text.into_bytes(),
        description: "values as json".to_string(),
        frame_kind: StreamingSendFrameKind::Text,
    })
}

pub(crate) fn streaming_script_send_request(
    method: &str,
    args: &[ParamValue],
) -> Option<Result<StreamingSendRequest, String>> {
    match method {
        "sendText" | "sendString" => {
            let text = args.first().and_then(ParamValue::as_str).unwrap_or_default();
            let line_ending = args
                .get(1)
                .and_then(ParamValue::as_str)
                .unwrap_or_else(|| LINE_ENDING_NONE.to_string());
            Some(Ok(string_request(text.as_str(), line_ending.as_str())))
        }
        "sendBytes" | "sendData" => {
            if args.len() == 1 {
                if let Some(text) = args[0].as_str() {
                    return Some(bytes_request(text.as_str()));
                }
            }
            Some(script_bytes_request(args))
        }
        "sendHex" | "sendHexString" => {
            let text = args.first().and_then(ParamValue::as_str).unwrap_or_default();
            Some(hex_string_request(text.as_str()))
        }
        _ => None,
    }
}

fn script_bytes_request(args: &[ParamValue]) -> Result<StreamingSendRequest, String> {
    Ok(StreamingSendRequest {
        bytes: script_bytes_from_args(args)?,
        description: "bytes".to_string(),
        frame_kind: StreamingSendFrameKind::Binary,
    })
}

fn script_bytes_from_args(args: &[ParamValue]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for value in args {
        if let Some(text) = value.as_str() {
            bytes.extend(parse_byte_list(text.as_str())?);
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
        let Some(value) = value
            .as_int()
            .or_else(|| value.as_float().map(|value| value.round() as i32))
        else {
            return Err("byte arguments must be numbers or byte-list strings".to_string());
        };
        bytes.push(script_i32_byte(value)?);
    }
    Ok(bytes)
}

fn script_f64_byte(value: f64) -> Result<u8, String> {
    script_i32_byte(value.round() as i32)
}

fn script_i32_byte(value: i32) -> Result<u8, String> {
    u8::try_from(value).map_err(|_| format!("byte {value} is outside the 0-255 range"))
}

pub(crate) fn value_to_streaming_string(value: &ParamValue) -> String {
    match value {
        ParamValue::Trigger() => "trigger".to_string(),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.clone(),
        _ => value.as_str().unwrap_or_else(|| value.to_string()),
    }
}

fn line_ending_bytes(line_ending: &str) -> &'static [u8] {
    match line_ending {
        LINE_ENDING_NL => b"\n",
        LINE_ENDING_CR => b"\r",
        LINE_ENDING_CRLF => b"\r\n",
        _ => b"",
    }
}

fn parse_byte_list(text: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    for token in text
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let value = parse_byte_token(token)?;
        output.push(value);
    }
    Ok(output)
}

fn parse_byte_token(token: &str) -> Result<u8, String> {
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|error| format!("invalid byte '{token}': {error}"));
    }

    token
        .parse::<u8>()
        .map_err(|error| format!("invalid byte '{token}': {error}"))
}

fn parse_hex_string(text: &str) -> Result<Vec<u8>, String> {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',' && *character != ';')
        .collect::<String>();

    if compact.len() % 2 != 0 {
        return Err("hex string must contain an even number of digits".to_string());
    }

    compact
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let pair = std::str::from_utf8(chunk).map_err(|error| format!("invalid hex string: {error}"))?;
            u8::from_str_radix(pair, 16).map_err(|error| format!("invalid hex byte '{pair}': {error}"))
        })
        .collect()
}

#[cfg(test)]
mod tests;
