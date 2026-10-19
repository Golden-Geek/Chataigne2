use super::*;
use crate::app::module::common::streaming::admission::{
    bounded_description, validate_path, MAX_JSON_INPUT_BYTES, MAX_STREAM_MESSAGES, MAX_STREAM_PATH_DEPTH,
    MAX_VALUE_UNITS,
};

// The input cap bounds the temporary serde DOM; this walk bounds conversion work and depth
// before any subtree is cloned by ParamValue conversion or expanded into graph paths.
fn validate_json_structure(value: &serde_json::Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
    *nodes += 1;
    if depth > MAX_STREAM_PATH_DEPTH || *nodes > 2 * MAX_VALUE_UNITS + MAX_STREAM_MESSAGES {
        return Err("stream JSON exceeds its nesting / preparation-work limit".into());
    }
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                validate_json_structure(value, depth + 1, nodes)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                validate_json_structure(value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn parse_json_bytes(bytes: &[u8]) -> Result<PreparedMessages, String> {
    if bytes.len() > MAX_JSON_INPUT_BYTES {
        return Err("stream JSON input exceeds 1 MiB".into());
    }
    if bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(PreparedMessages::default());
    }

    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid JSON input: {error}"))?;

    validate_json_structure(&value, 0, &mut 0)?;
    let mut messages = MessagePreparation::default();
    let mut path_segments = Vec::new();
    collect_json_messages(&value, &mut path_segments, &mut messages)?;
    messages.finish()
}

fn collect_json_messages(
    value: &serde_json::Value,
    path_segments: &mut Vec<String>,
    messages: &mut MessagePreparation,
) -> Result<(), String> {
    validate_path(path_segments)?;
    if let Some(payload) = decode_json_payload(value)? {
        messages.try_push(ReceivedValueMessage {
            path_segments: json_message_path_segments(path_segments.as_slice()),
            payload,
            source_description: json_source_description(path_segments.as_slice()),
        })?;
        return Ok(());
    }

    match value {
        serde_json::Value::Object(entries) => {
            for (key, child) in entries {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }

                path_segments.push(key.to_string());
                collect_json_messages(child, path_segments, messages)?;
                path_segments.pop();
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path_segments.push((index + 1).to_string());
                collect_json_messages(child, path_segments, messages)?;
                path_segments.pop();
            }
        }
        _ => {}
    }

    Ok(())
}

fn decode_json_payload(value: &serde_json::Value) -> Result<Option<ReceivedValuePayload>, String> {
    match value {
        serde_json::Value::Object(_) => Ok(None),
        serde_json::Value::Array(items) => {
            if items.len() > MAX_VALUE_UNITS {
                return Err("stream JSON list exceeds the 16 MiB value budget".into());
            }
            if let Ok(value) = ParamValue::from_script_json(value) {
                return Ok(Some(ReceivedValuePayload::Single(value)));
            }

            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let Ok(value) = ParamValue::from_script_json(item) else {
                    return Ok(None);
                };
                values.push(value);
            }

            Ok(Some(match values.len() {
                0 => ReceivedValuePayload::Single(ParamValue::Trigger()),
                1 => ReceivedValuePayload::Single(values.remove(0)),
                _ => ReceivedValuePayload::Multi(values),
            }))
        }
        _ => ParamValue::from_script_json(value)
            .map(ReceivedValuePayload::Single)
            .map(Some)
            .map_err(|error| format!("invalid JSON value: {error}")),
    }
}

fn json_message_path_segments(path_segments: &[String]) -> Vec<String> {
    if path_segments.is_empty() {
        vec![DEFAULT_LINE_VALUE_NAME.to_string()]
    } else {
        path_segments.to_vec()
    }
}

fn json_source_description(path_segments: &[String]) -> String {
    if path_segments.is_empty() {
        "streaming json root".to_string()
    } else {
        bounded_description("streaming json path '", &path_segments.join("/"), "'")
    }
}
