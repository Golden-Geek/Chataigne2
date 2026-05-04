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
mod commands_tests;
