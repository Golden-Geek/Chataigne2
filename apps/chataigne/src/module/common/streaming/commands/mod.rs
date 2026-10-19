mod bounds;
mod capture;
mod outgoing;
mod script_input;
pub(crate) use outgoing::{OutgoingBytes, SendInput, StreamSend};
pub(crate) use script_input::ScriptSend;
pub(crate) mod preparation;
use bounds::{source, Output};
pub(crate) use bounds::{validate_values, value_weight, MAX_COMMAND_BYTES, MAX_COMMAND_VALUES};
pub(crate) use capture::CommandCapture;
use golden_core::parameter::ParamValue;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, io::Write};

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

pub(crate) fn string_request(text: &str, line_ending: &str) -> Result<StreamingSendRequest, String> {
    source(text)?;
    source(line_ending)?;
    let mut output = decode_text_escape_sequences(text).into_bytes();
    let ending = line_ending_bytes(line_ending);
    if output.len().saturating_add(ending.len()) > MAX_COMMAND_BYTES {
        return Err("Stream command output exceeds 1 MiB".into());
    }
    output.extend_from_slice(ending);
    output.shrink_to_fit();
    Ok(StreamingSendRequest {
        bytes: output,
        description: "string".to_string(),
        frame_kind: StreamingSendFrameKind::Text,
    })
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

pub(crate) fn values_request<'a>(
    values: impl IntoIterator<Item = &'a ParamValue>,
    prefix: &str,
    suffix: &str,
    separator: &str,
) -> Result<StreamingSendRequest, String> {
    let mut source_bytes = 0_usize;
    for text in [prefix, suffix, separator] {
        source(text)?;
        source_bytes = source_bytes.saturating_add(text.len());
    }
    if source_bytes > bounds::MAX_SOURCE_BYTES {
        return Err("Stream command source exceeds 4 MiB".into());
    }
    let mut output = Output::default();
    output
        .write_all(decode_text_escape_sequences(prefix).as_bytes())
        .map_err(|error| error.to_string())?;
    let separator = decode_text_escape_sequences(separator);
    for (index, value) in values.into_iter().enumerate() {
        source_bytes = source_bytes.saturating_add(value_weight(value));
        if index >= MAX_COMMAND_VALUES || source_bytes > bounds::MAX_SOURCE_BYTES {
            return Err("Stream command exceeds 4096 values or 4 MiB".into());
        }
        if index > 0 {
            output
                .write_all(separator.as_bytes())
                .map_err(|error| error.to_string())?;
        }
        output
            .write_all(value_to_streaming_string(value).as_bytes())
            .map_err(|error| error.to_string())?;
    }
    output
        .write_all(decode_text_escape_sequences(suffix).as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(StreamingSendRequest {
        bytes: output.0,
        description: "values".into(),
        frame_kind: StreamingSendFrameKind::Binary,
    })
}

pub(crate) fn values_json_request(value: &serde_json::Value) -> Result<StreamingSendRequest, String> {
    bounds::json_weight(value, bounds::MAX_SOURCE_BYTES, 16_384, 32)?;
    let mut output = Output::default();
    serde_json::to_writer(&mut output, value)
        .map_err(|error| format!("failed to encode bounded values JSON: {error}"))?;
    Ok(StreamingSendRequest {
        bytes: output.0,
        description: "values as json".into(),
        frame_kind: StreamingSendFrameKind::Text,
    })
}

pub(crate) fn streaming_script_send_request(
    method: &str,
    args: &[ParamValue],
) -> Option<Result<StreamingSendRequest, String>> {
    if !matches!(
        method,
        "sendText" | "sendString" | "sendBytes" | "sendData" | "sendHex" | "sendHexString"
    ) {
        return None;
    }
    if let Err(error) = validate_values(args) {
        return Some(Err(error));
    }
    match method {
        "sendText" | "sendString" => {
            let text = args.first().and_then(ParamValue::as_str).unwrap_or_default();
            let line_ending = args
                .get(1)
                .and_then(ParamValue::as_str)
                .unwrap_or_else(|| LINE_ENDING_NONE.to_string());
            Some(string_request(text.as_str(), line_ending.as_str()))
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
    let mut bytes = Output::default();
    for value in args {
        if let Some(text) = value.as_str() {
            let parsed = parse_byte_list(text.as_str())?;
            bytes.write_all(&parsed).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some((x, y)) = value.as_vec2() {
            bytes
                .write_all(&[script_f64_byte(x)?, script_f64_byte(y)?])
                .map_err(|error| error.to_string())?;
            continue;
        }
        if let Some((x, y, z)) = value.as_vec3() {
            bytes
                .write_all(&[script_f64_byte(x)?, script_f64_byte(y)?, script_f64_byte(z)?])
                .map_err(|error| error.to_string())?;
            continue;
        }
        let Some(value) = value
            .as_int()
            .or_else(|| value.as_float().map(|value| value.round() as i32))
        else {
            return Err("byte arguments must be numbers or byte-list strings".to_string());
        };
        bytes
            .write_all(&[script_i32_byte(value)?])
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes.0)
}

fn script_f64_byte(value: f64) -> Result<u8, String> {
    script_i32_byte(value.round() as i32)
}

fn script_i32_byte(value: i32) -> Result<u8, String> {
    u8::try_from(value).map_err(|_| format!("byte {value} is outside the 0-255 range"))
}

fn value_to_streaming_string(value: &ParamValue) -> Cow<'_, str> {
    match value {
        ParamValue::Trigger() => Cow::Borrowed("trigger"),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => Cow::Borrowed(value),
        _ => Cow::Owned(value.as_str().unwrap_or_else(|| value.to_string())),
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
    source(text)?;
    let mut output = Vec::new();
    for token in text
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let value = parse_byte_token(token)?;
        if output.len() == MAX_COMMAND_BYTES {
            return Err("Stream byte command exceeds 1 MiB".into());
        }
        output.push(value);
    }
    Ok(output)
}

fn parse_byte_token(token: &str) -> Result<u8, String> {
    if token.len() > 32 {
        return Err("Stream byte token exceeds 32 bytes".into());
    }
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|error| format!("invalid byte '{token}': {error}"));
    }

    token
        .parse::<u8>()
        .map_err(|error| format!("invalid byte '{token}': {error}"))
}

fn parse_hex_string(text: &str) -> Result<Vec<u8>, String> {
    source(text)?;
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ',' && *character != ';')
        .collect::<String>();

    if compact.len() % 2 != 0 {
        return Err("hex string must contain an even number of digits".to_string());
    }

    if compact.len() / 2 > MAX_COMMAND_BYTES {
        return Err("Stream hex command exceeds 1 MiB".into());
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
