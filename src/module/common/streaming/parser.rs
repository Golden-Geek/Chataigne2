use golden_core::parameter::ParamValue;

use crate::app::module::common::received_values::ReceivedValuePayload;

pub(crate) const STREAMING_INPUT_MODE_RAW: &str = "raw";
pub(crate) const STREAMING_INPUT_MODE_LINE: &str = "line";
pub(crate) const STREAMING_INPUT_MODE_JSON: &str = "json";
pub(crate) const STREAMING_SEPARATOR_COMMA: &str = "comma";
pub(crate) const STREAMING_SEPARATOR_SPACE: &str = "space";
pub(crate) const STREAMING_SEPARATOR_COLON: &str = "colon";
pub(crate) const STREAMING_SEPARATOR_SEMICOLON: &str = "semicolon";
pub(crate) const STREAMING_SEPARATOR_DOT: &str = "dot";
pub(crate) const STREAMING_SEPARATOR_TAB: &str = "tab";
pub(crate) const DEFAULT_RAW_VALUE_NAME: &str = "byte";
pub(crate) const DEFAULT_LINE_VALUE_NAME: &str = "received";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StreamingSeparator {
    Comma,
    Space,
    Colon,
    Semicolon,
    Dot,
    Tab,
}

impl StreamingSeparator {
    pub(crate) fn from_variant(variant: &str) -> Option<Self> {
        match variant {
            STREAMING_SEPARATOR_COMMA | "," => Some(Self::Comma),
            STREAMING_SEPARATOR_SPACE | " " => Some(Self::Space),
            STREAMING_SEPARATOR_COLON | ":" => Some(Self::Colon),
            STREAMING_SEPARATOR_SEMICOLON | ";" => Some(Self::Semicolon),
            STREAMING_SEPARATOR_DOT | "." => Some(Self::Dot),
            STREAMING_SEPARATOR_TAB | "\\t" | "\t" => Some(Self::Tab),
            _ => None,
        }
    }

    fn character(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Space => ' ',
            Self::Colon => ':',
            Self::Semicolon => ';',
            Self::Dot => '.',
            Self::Tab => '\t',
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StreamingParseConfig {
    pub mode: String,
    pub name_separator: Option<StreamingSeparator>,
    pub value_separator: Option<StreamingSeparator>,
    pub hierarchy_separator: Option<StreamingSeparator>,
}

impl Default for StreamingParseConfig {
    fn default() -> Self {
        Self {
            mode: STREAMING_INPUT_MODE_LINE.to_string(),
            name_separator: Some(StreamingSeparator::Space),
            value_separator: Some(StreamingSeparator::Colon),
            hierarchy_separator: Some(StreamingSeparator::Dot),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StreamingIncomingMessage {
    pub path_segments: Vec<String>,
    pub payload: ReceivedValuePayload,
    pub source_description: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StreamingParser {
    pending_line_bytes: Vec<u8>,
    skip_lf_after_cr: bool,
}

impl StreamingParser {
    pub(crate) fn push_bytes(
        &mut self,
        bytes: &[u8],
        config: &StreamingParseConfig,
    ) -> Result<Vec<StreamingIncomingMessage>, String> {
        if config.mode == STREAMING_INPUT_MODE_RAW {
            self.pending_line_bytes.clear();
            self.skip_lf_after_cr = false;
            return Ok(bytes
                .iter()
                .map(|byte| StreamingIncomingMessage {
                    path_segments: vec![DEFAULT_RAW_VALUE_NAME.to_string()],
                    payload: ReceivedValuePayload::Single(ParamValue::Int(i32::from(*byte))),
                    source_description: format!("raw byte 0x{byte:02X}"),
                })
                .collect());
        }

            if config.mode == STREAMING_INPUT_MODE_JSON {
                self.pending_line_bytes.clear();
                self.skip_lf_after_cr = false;
                return parse_json_bytes(bytes);
            }

        let bytes = self.trim_paired_lf_after_split_cr(bytes);

        self.pending_line_bytes.extend_from_slice(bytes);

        let mut messages = Vec::new();
        while let Some((delimiter_index, delimiter_len, skip_lf_after_cr)) =
            find_line_break(self.pending_line_bytes.as_slice())
        {
            let line_bytes = self.pending_line_bytes[..delimiter_index].to_vec();
            self.pending_line_bytes.drain(..delimiter_index + delimiter_len);
            self.skip_lf_after_cr = skip_lf_after_cr;
            if let Some(message) = parse_line(line_bytes.as_slice(), config) {
                messages.push(message);
            }
        }

        Ok(messages)
    }

    fn trim_paired_lf_after_split_cr<'a>(&mut self, bytes: &'a [u8]) -> &'a [u8] {
        if !self.skip_lf_after_cr {
            return bytes;
        }

        self.skip_lf_after_cr = false;
        if bytes.first() == Some(&b'\n') {
            &bytes[1..]
        } else {
            bytes
        }
    }
}

fn parse_json_bytes(bytes: &[u8]) -> Result<Vec<StreamingIncomingMessage>, String> {
    if bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(Vec::new());
    }

    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid JSON input: {error}"))?;

    let mut messages = Vec::new();
    let mut path_segments = Vec::new();
    collect_json_messages(&value, &mut path_segments, &mut messages)?;
    Ok(messages)
}

fn collect_json_messages(
    value: &serde_json::Value,
    path_segments: &mut Vec<String>,
    messages: &mut Vec<StreamingIncomingMessage>,
) -> Result<(), String> {
    if let Some(payload) = decode_json_payload(value)? {
        messages.push(StreamingIncomingMessage {
            path_segments: json_message_path_segments(path_segments.as_slice()),
            payload,
            source_description: json_source_description(path_segments.as_slice()),
        });
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
        format!("streaming json path '{}'", path_segments.join("/"))
    }
}

pub(crate) fn parse_line(line_bytes: &[u8], config: &StreamingParseConfig) -> Option<StreamingIncomingMessage> {
    let line = String::from_utf8_lossy(line_bytes);
    let line = line.trim_matches(|character| character == '\r' || character == '\n');
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let (name, value_text) = name_and_value_text(line, config.name_separator)?;
    if name.is_empty() {
        return None;
    }

    let values = split_value_fields(value_text, config.value_separator);
    let path_segments = name_segments(name.as_str(), config.hierarchy_separator);
    if path_segments.is_empty() {
        return None;
    }

    Some(StreamingIncomingMessage {
        source_description: format!("streaming line '{line}'"),
        path_segments,
        payload: decode_value_fields(values.as_slice()),
    })
}

pub(crate) fn decode_text_escape_sequences(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }

        match chars.next() {
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('t') => output.push('\t'),
            Some('0') => output.push('\0'),
            Some('\\') => output.push('\\'),
            Some('x') => {
                let high = chars.next();
                let low = chars.next();
                match (high, low) {
                    (Some(high), Some(low)) => {
                        let digits = [high, low].iter().collect::<String>();
                        if let Ok(value) = u8::from_str_radix(digits.as_str(), 16) {
                            output.push(char::from(value));
                        } else {
                            output.push_str("\\x");
                            output.push(high);
                            output.push(low);
                        }
                    }
                    (Some(high), None) => {
                        output.push_str("\\x");
                        output.push(high);
                    }
                    (None, _) => output.push_str("\\x"),
                }
            }
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }

    output
}

pub(crate) fn decode_value_fields(fields: &[String]) -> ReceivedValuePayload {
    if fields.is_empty() {
        return ReceivedValuePayload::Single(ParamValue::Trigger());
    }

    if fields.len() == 1 {
        return ReceivedValuePayload::Single(parse_scalar_value(fields[0].as_str()));
    }

    if let Some(value) = decode_vector_like_value(fields) {
        return ReceivedValuePayload::Single(value);
    }

    ReceivedValuePayload::Multi(fields.iter().map(|field| parse_scalar_value(field.as_str())).collect())
}

pub(crate) fn parse_scalar_value(input: &str) -> ParamValue {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ParamValue::Str(String::new());
    }

    if let Some(value) = parse_quoted_string(trimmed) {
        return ParamValue::Str(value);
    }

    if trimmed.eq_ignore_ascii_case("true") {
        return ParamValue::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return ParamValue::Bool(false);
    }

    if let Some(value) = parse_hex_int(trimmed) {
        return ParamValue::Int(value);
    }

    if let Ok(value) = trimmed.parse::<i32>() {
        return ParamValue::Int(value);
    }

    if let Ok(value) = trimmed.parse::<f64>() {
        return ParamValue::Float(value);
    }

    ParamValue::Str(trimmed.to_string())
}

fn name_and_value_text(line: &str, name_separator: Option<StreamingSeparator>) -> Option<(String, &str)> {
    let Some(separator) = name_separator else {
        return Some((DEFAULT_LINE_VALUE_NAME.to_string(), line));
    };

    if let Some((name, value_text)) = split_once_by_separator(line, separator) {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        return Some((name.to_string(), value_text.trim()));
    }

    Some((line.trim().to_string(), ""))
}

fn decode_vector_like_value(fields: &[String]) -> Option<ParamValue> {
    let numeric_values = fields
        .iter()
        .map(|field| parse_numeric_field(field.as_str()))
        .collect::<Option<Vec<_>>>()?;

    match numeric_values.as_slice() {
        [x, y] => Some(ParamValue::Vec2(*x, *y)),
        [x, y, z] => Some(ParamValue::Vec3(*x, *y, *z)),
        [r, g, b, a] => {
            if fields.iter().all(|field| is_integer_text(field.as_str())) {
                Some(ParamValue::Color(
                    (*r / 255.0).clamp(0.0, 1.0),
                    (*g / 255.0).clamp(0.0, 1.0),
                    (*b / 255.0).clamp(0.0, 1.0),
                    (*a / 255.0).clamp(0.0, 1.0),
                ))
            } else {
                Some(ParamValue::Color(*r, *g, *b, *a))
            }
        }
        _ => None,
    }
}

fn split_value_fields(text: &str, separator: Option<StreamingSeparator>) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let Some(separator) = separator else {
        return vec![text.to_string()];
    };

    split_all_by_separator(text, separator)
}

fn name_segments(name: &str, hierarchy_separator: Option<StreamingSeparator>) -> Vec<String> {
    let slash_segments = name
        .trim()
        .trim_matches('/')
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty());

    let mut output = Vec::new();
    for slash_segment in slash_segments {
        if let Some(separator) = hierarchy_separator {
            output.extend(split_all_by_separator(slash_segment, separator));
        } else {
            output.push(slash_segment.to_string());
        }
    }

    output
}

fn split_once_by_separator(input: &str, separator: StreamingSeparator) -> Option<(&str, &str)> {
    let separator = separator.character();
    let index = input.find(separator)?;
    let value_start = input[index..]
        .char_indices()
        .find(|(_, character)| *character != separator)
        .map(|(offset, _)| index + offset)
        .unwrap_or(input.len());

    Some((&input[..index], &input[value_start..]))
}

fn split_all_by_separator(input: &str, separator: StreamingSeparator) -> Vec<String> {
    input
        .split(separator.character())
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_numeric_field(input: &str) -> Option<f64> {
    let trimmed = input.trim();
    if let Some(value) = parse_hex_int(trimmed) {
        return Some(f64::from(value));
    }
    trimmed.parse::<f64>().ok()
}

fn parse_hex_int(input: &str) -> Option<i32> {
    let without_prefix = input.strip_prefix("0x").or_else(|| input.strip_prefix("0X"))?;
    i32::from_str_radix(without_prefix, 16).ok()
}

fn parse_quoted_string(input: &str) -> Option<String> {
    let mut chars = input.chars();
    let first = chars.next()?;
    let last = input.chars().last()?;
    if input.len() < 2 || !((first == '"' && last == '"') || (first == '\'' && last == '\'')) {
        return None;
    }

    Some(decode_text_escape_sequences(&input[1..input.len() - 1]))
}

fn is_integer_text(input: &str) -> bool {
    let trimmed = input.trim();
    parse_hex_int(trimmed).is_some() || trimmed.parse::<i32>().is_ok()
}

fn find_line_break(bytes: &[u8]) -> Option<(usize, usize, bool)> {
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'\n' => return Some((index, 1, false)),
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    return Some((index, 2, false));
                }
                return Some((index, 1, index + 1 == bytes.len()));
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod parser_tests;
