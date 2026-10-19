mod json;

use super::admission::{bounded_description, validate_path, PreparedMessages, MAX_STREAM_MESSAGES, MAX_VALUE_UNITS};
use json::parse_json_bytes;

use golden_core::parameter::ParamValue;

use crate::app::module::common::received_values::{ReceivedValueMessage, ReceivedValuePayload};

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
const MAX_PENDING_LINE_BYTES: usize = 1024 * 1024;

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

#[derive(Clone, Debug, Default)]
pub(crate) struct StreamingParser {
    pending_line_bytes: Vec<u8>,
    skip_lf_after_cr: bool,
    discarding_line: bool,
    active_config: Option<StreamingParseConfig>,
    rejected_lines: u64,
}

impl StreamingParser {
    pub(crate) fn push_bytes(
        &mut self,
        bytes: &[u8],
        config: &StreamingParseConfig,
    ) -> Result<PreparedMessages, String> {
        if self.active_config.as_ref() != Some(config) {
            if !self.pending_line_bytes.is_empty() {
                self.rejected_lines = self.rejected_lines.saturating_add(1);
            }
            self.pending_line_bytes.clear();
            self.skip_lf_after_cr = false;
            self.discarding_line = false;
            self.active_config = Some(config.clone());
        }
        if config.mode == STREAMING_INPUT_MODE_RAW {
            self.pending_line_bytes.clear();
            self.skip_lf_after_cr = false;
            if bytes.len() > MAX_STREAM_MESSAGES {
                return Err("raw stream input exceeds 65536 byte messages".into());
            }
            let mut messages = PreparedMessages::default();
            for byte in bytes {
                messages.try_push(ReceivedValueMessage {
                    path_segments: vec![DEFAULT_RAW_VALUE_NAME.to_string()],
                    payload: ReceivedValuePayload::Single(ParamValue::Int(i32::from(*byte))),
                    source_description: format!("raw byte 0x{byte:02X}"),
                })?;
            }
            return Ok(messages);
        }

        if config.mode == STREAMING_INPUT_MODE_JSON {
            self.pending_line_bytes.clear();
            self.skip_lf_after_cr = false;
            return parse_json_bytes(bytes);
        }

        let mut messages = PreparedMessages::default();
        for part in bytes.split_inclusive(|byte| matches!(byte, b'\r' | b'\n')) {
            if self.skip_lf_after_cr {
                self.skip_lf_after_cr = false;
                if part == b"\n" {
                    continue;
                }
            }
            let delimiter = part.last().copied().filter(|byte| matches!(byte, b'\r' | b'\n'));
            let content = &part[..part.len() - usize::from(delimiter.is_some())];
            if !self.discarding_line {
                if content.len() <= MAX_PENDING_LINE_BYTES - self.pending_line_bytes.len() {
                    let needed = self.pending_line_bytes.len() + content.len();
                    if needed > self.pending_line_bytes.capacity() {
                        // Geometric growth within the cap also bounds copies across tiny chunks.
                        let capacity = self
                            .pending_line_bytes
                            .capacity()
                            .saturating_mul(2)
                            .max(needed)
                            .clamp(1024, MAX_PENDING_LINE_BYTES);
                        self.pending_line_bytes
                            .reserve_exact(capacity - self.pending_line_bytes.len());
                    }
                    self.pending_line_bytes.extend_from_slice(content);
                } else {
                    self.pending_line_bytes = Vec::new();
                    self.discarding_line = true;
                    self.rejected_lines = self.rejected_lines.saturating_add(1);
                }
            }
            if let Some(delimiter) = delimiter {
                if !self.discarding_line {
                    let result = parse_line(&self.pending_line_bytes, config)
                        .and_then(|message| message.map_or(Ok(()), |message| messages.try_push(message)));
                    if result.is_err() {
                        self.rejected_lines = self.rejected_lines.saturating_add(1);
                    }
                }
                self.pending_line_bytes.clear();
                self.discarding_line = false;
                self.skip_lf_after_cr = delimiter == b'\r';
            }
        }

        Ok(messages)
    }

    pub(crate) fn take_rejected_line_count(&mut self) -> u64 {
        std::mem::take(&mut self.rejected_lines)
    }
}

fn parse_line(line_bytes: &[u8], config: &StreamingParseConfig) -> Result<Option<ReceivedValueMessage>, String> {
    let line = String::from_utf8_lossy(line_bytes);
    let line = line.trim_matches(|character| character == '\r' || character == '\n');
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }

    let Some((name, value_text)) = name_and_value_text(line, config.name_separator) else {
        return Ok(None);
    };
    if name.is_empty() {
        return Ok(None);
    }

    let path_segments = name_segments(name.as_str(), config.hierarchy_separator)?;
    validate_path(&path_segments)?;
    let values = split_value_fields(value_text, config.value_separator)?;
    if path_segments.is_empty() {
        return Ok(None);
    }

    Ok(Some(ReceivedValueMessage {
        source_description: bounded_description("streaming line '", line, "'"),
        path_segments,
        payload: decode_value_fields(values.as_slice()),
    }))
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

fn split_value_fields(text: &str, separator: Option<StreamingSeparator>) -> Result<Vec<String>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }

    let Some(separator) = separator else {
        return Ok(vec![text.to_string()]);
    };

    split_all_by_separator(text, separator, MAX_VALUE_UNITS)
}

fn name_segments(name: &str, hierarchy_separator: Option<StreamingSeparator>) -> Result<Vec<String>, String> {
    if name.len() > super::admission::MAX_STREAM_PATH_BYTES {
        return Err("stream path exceeds 4096 UTF-8 bytes".into());
    }
    let slash_segments = name
        .trim()
        .trim_matches('/')
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty());

    let mut output = Vec::new();
    for slash_segment in slash_segments {
        let remaining = super::admission::MAX_STREAM_PATH_DEPTH - output.len();
        if let Some(separator) = hierarchy_separator {
            output.extend(split_all_by_separator(slash_segment, separator, remaining)?);
        } else {
            if remaining == 0 {
                return Err("stream path exceeds 64 levels".into());
            }
            output.push(slash_segment.to_string());
        }
    }

    validate_path(&output)?;
    Ok(output)
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

fn split_all_by_separator(input: &str, separator: StreamingSeparator, limit: usize) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    for field in input
        .split(separator.character())
        .map(str::trim)
        .filter(|field| !field.is_empty())
    {
        if fields.len() == limit {
            return Err("stream field expansion exceeds its preparation limit".into());
        }
        fields.push(field.to_string());
    }
    Ok(fields)
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

#[cfg(test)]
mod tests;
