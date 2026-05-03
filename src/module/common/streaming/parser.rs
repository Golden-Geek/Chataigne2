use golden_core::parameter::ParamValue;

use crate::app::module::common::received_values::ReceivedValuePayload;

pub(crate) const STREAMING_INPUT_MODE_RAW: &str = "raw";
pub(crate) const STREAMING_INPUT_MODE_LINE: &str = "line";
pub(crate) const STREAMING_FIRST_ELEMENT_VALUE: &str = "value";
pub(crate) const STREAMING_FIRST_ELEMENT_NAME: &str = "name";
pub(crate) const DEFAULT_RAW_VALUE_NAME: &str = "byte";
pub(crate) const DEFAULT_LINE_VALUE_NAME: &str = "received";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StreamingParseConfig {
    pub mode: String,
    pub line_delimiter: String,
    pub value_separator: String,
    pub first_element: String,
    pub hierarchy_from_name: bool,
    pub hierarchy_delimiter: String,
}

impl Default for StreamingParseConfig {
    fn default() -> Self {
        Self {
            mode: STREAMING_INPUT_MODE_LINE.to_string(),
            line_delimiter: "\n".to_string(),
            value_separator: ",".to_string(),
            first_element: STREAMING_FIRST_ELEMENT_NAME.to_string(),
            hierarchy_from_name: true,
            hierarchy_delimiter: ".".to_string(),
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
}

impl StreamingParser {
    pub(crate) fn push_bytes(
        &mut self,
        bytes: &[u8],
        config: &StreamingParseConfig,
    ) -> Result<Vec<StreamingIncomingMessage>, String> {
        if config.mode == STREAMING_INPUT_MODE_RAW {
            self.pending_line_bytes.clear();
            return Ok(bytes
                .iter()
                .map(|byte| StreamingIncomingMessage {
                    path_segments: vec![DEFAULT_RAW_VALUE_NAME.to_string()],
                    payload: ReceivedValuePayload::Single(ParamValue::Int(i32::from(*byte))),
                    source_description: format!("raw byte 0x{byte:02X}"),
                })
                .collect());
        }

        let delimiter = config.line_delimiter.as_bytes();
        if delimiter.is_empty() {
            return Err("line delimiter cannot be empty".to_string());
        }

        self.pending_line_bytes.extend_from_slice(bytes);

        let mut messages = Vec::new();
        while let Some(delimiter_index) = find_subslice(self.pending_line_bytes.as_slice(), delimiter) {
            let line_bytes = self.pending_line_bytes[..delimiter_index].to_vec();
            self.pending_line_bytes
                .drain(..delimiter_index + delimiter.len());
            if let Some(message) = parse_line(line_bytes.as_slice(), config) {
                messages.push(message);
            }
        }

        Ok(messages)
    }
}

pub(crate) fn parse_line(line_bytes: &[u8], config: &StreamingParseConfig) -> Option<StreamingIncomingMessage> {
    let line = String::from_utf8_lossy(line_bytes);
    let line = line.trim_matches(|character| character == '\r' || character == '\n');
    if line.trim().is_empty() {
        return None;
    }

    let fields = split_fields(line, config.value_separator.as_str());
    if fields.is_empty() {
        return None;
    }

    let (name, values) = if config.first_element == STREAMING_FIRST_ELEMENT_NAME {
        let name = fields[0].trim();
        if name.is_empty() {
            return None;
        }
        (name.to_string(), fields[1..].to_vec())
    } else if config.first_element == STREAMING_FIRST_ELEMENT_VALUE {
        (DEFAULT_LINE_VALUE_NAME.to_string(), fields)
    } else {
        (DEFAULT_LINE_VALUE_NAME.to_string(), fields)
    };

    let path_segments = name_segments(
        name.as_str(),
        config.hierarchy_from_name,
        config.hierarchy_delimiter.as_str(),
    );
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

    ReceivedValuePayload::Multi(
        fields
            .iter()
            .map(|field| parse_scalar_value(field.as_str()))
            .collect(),
    )
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

fn split_fields(line: &str, separator: &str) -> Vec<String> {
    if separator.is_empty() {
        return line
            .split_whitespace()
            .map(str::trim)
            .filter(|field| !field.is_empty())
            .map(str::to_string)
            .collect();
    }

    line.split(separator).map(str::trim).map(str::to_string).collect()
}

fn name_segments(name: &str, hierarchy_from_name: bool, hierarchy_delimiter: &str) -> Vec<String> {
    let slash_segments = name
        .trim()
        .trim_matches('/')
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty());

    let mut output = Vec::new();
    for slash_segment in slash_segments {
        if hierarchy_from_name && !hierarchy_delimiter.is_empty() {
            output.extend(
                slash_segment
                    .split(hierarchy_delimiter)
                    .map(str::trim)
                    .filter(|segment| !segment.is_empty())
                    .map(str::to_string),
            );
        } else {
            output.push(slash_segment.to_string());
        }
    }

    output
}

fn parse_numeric_field(input: &str) -> Option<f64> {
    let trimmed = input.trim();
    if let Some(value) = parse_hex_int(trimmed) {
        return Some(f64::from(value));
    }
    trimmed.parse::<f64>().ok()
}

fn parse_hex_int(input: &str) -> Option<i32> {
    let without_prefix = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))?;
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

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use golden_core::parameter::ParamValue;

    use super::*;

    #[test]
    fn line_parser_splits_named_hierarchy() {
        let config = StreamingParseConfig::default();
        let message = parse_line(b"my.received.value,42\n", &config).expect("line should parse");

        assert_eq!(message.path_segments, vec!["my", "received", "value"]);
        assert_eq!(
            message.payload,
            ReceivedValuePayload::Single(ParamValue::Int(42))
        );
    }

    #[test]
    fn line_parser_uses_default_name_for_value_lines() {
        let config = StreamingParseConfig {
            first_element: STREAMING_FIRST_ELEMENT_VALUE.to_string(),
            ..StreamingParseConfig::default()
        };
        let message = parse_line(b"1.5,2.5\n", &config).expect("line should parse");

        assert_eq!(message.path_segments, vec![DEFAULT_LINE_VALUE_NAME]);
        assert_eq!(
            message.payload,
            ReceivedValuePayload::Single(ParamValue::Vec2(1.5, 2.5))
        );
    }

    #[test]
    fn raw_mode_emits_one_message_per_byte() {
        let mut parser = StreamingParser::default();
        let config = StreamingParseConfig {
            mode: STREAMING_INPUT_MODE_RAW.to_string(),
            ..StreamingParseConfig::default()
        };

        let messages = parser
            .push_bytes(&[0x01, 0xFF], &config)
            .expect("raw bytes should parse");

        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].payload,
            ReceivedValuePayload::Single(ParamValue::Int(1))
        );
        assert_eq!(
            messages[1].payload,
            ReceivedValuePayload::Single(ParamValue::Int(255))
        );
    }
}
