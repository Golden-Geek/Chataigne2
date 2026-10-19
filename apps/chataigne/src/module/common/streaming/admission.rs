use std::{mem::size_of, ops::Deref};

use golden_core::parameter::ParamValue;

use crate::app::module::common::received_values::{ReceivedValueMessage, ReceivedValuePayload};

pub(crate) const MAX_STREAM_MESSAGES: usize = 65_536;
pub(crate) const MAX_STREAM_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_STREAM_PATH_DEPTH: usize = 64;
pub(crate) const MAX_STREAM_PATH_BYTES: usize = 4096;
pub(super) const MAX_VALUE_UNITS: usize = MAX_STREAM_BYTES / size_of::<ParamValue>();
pub(super) const MAX_JSON_INPUT_BYTES: usize = 1024 * 1024;

/// Prepared on the input boundary. Consumers cannot mutate the admitted values or their weight.
#[derive(Debug, Default)]
pub(crate) struct PreparedMessages {
    messages: Vec<ReceivedValueMessage>,
    dynamic_bytes: usize,
    value_units: usize,
}

impl PreparedMessages {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.messages
            .capacity()
            .saturating_mul(size_of::<ReceivedValueMessage>())
            .saturating_add(self.dynamic_bytes)
    }

    pub(super) fn try_push(&mut self, message: ReceivedValueMessage) -> Result<(), String> {
        validate_path(&message.path_segments)?;
        if self.messages.len() == MAX_STREAM_MESSAGES {
            return Err("stream preparation exceeds 65536 messages".into());
        }
        let units = self.value_units.saturating_add(value_units(&message.payload));
        if units > MAX_VALUE_UNITS {
            return Err("stream values exceed the 16 MiB preparation budget".into());
        }
        let dynamic_bytes = self.dynamic_bytes.saturating_add(message.retained_weight());
        let needed = self.messages.len() + 1;
        let mut capacity = self.messages.capacity();
        if needed > capacity {
            capacity = capacity.saturating_mul(2).max(needed).clamp(4, MAX_STREAM_MESSAGES);
        }
        if dynamic_bytes.saturating_add(capacity * size_of::<ReceivedValueMessage>()) > MAX_STREAM_BYTES {
            // A geometric reservation must not reject a prefix that fits with exact storage.
            capacity = needed.max(self.messages.capacity());
            if dynamic_bytes.saturating_add(capacity * size_of::<ReceivedValueMessage>()) > MAX_STREAM_BYTES {
                return Err("stream expansion exceeds 16 MiB of retained storage".into());
            }
        }
        if capacity > self.messages.capacity() {
            self.messages.reserve_exact(capacity - self.messages.len());
        }
        self.messages.push(message);
        self.dynamic_bytes = dynamic_bytes;
        self.value_units = units;
        Ok(())
    }
}

impl TryFrom<Vec<ReceivedValueMessage>> for PreparedMessages {
    type Error = String;

    fn try_from(messages: Vec<ReceivedValueMessage>) -> Result<Self, Self::Error> {
        if messages.len() > MAX_STREAM_MESSAGES {
            return Err("stream preparation exceeds 65536 messages".into());
        }
        let mut dynamic_bytes = 0usize;
        let mut units = 0usize;
        for message in &messages {
            validate_path(&message.path_segments)?;
            dynamic_bytes = dynamic_bytes.saturating_add(message.retained_weight());
            units = units.saturating_add(value_units(&message.payload));
        }
        let prepared = Self {
            messages,
            dynamic_bytes,
            value_units: units,
        };
        if units > MAX_VALUE_UNITS || prepared.retained_bytes() > MAX_STREAM_BYTES {
            return Err("stream expansion exceeds 16 MiB of retained storage".into());
        }
        Ok(prepared)
    }
}

impl Deref for PreparedMessages {
    type Target = [ReceivedValueMessage];

    fn deref(&self) -> &Self::Target {
        &self.messages
    }
}

impl IntoIterator for PreparedMessages {
    type Item = ReceivedValueMessage;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.into_iter()
    }
}

fn value_units(payload: &ReceivedValuePayload) -> usize {
    match payload {
        ReceivedValuePayload::Single(_) => 1,
        ReceivedValuePayload::Multi(values) => values.len().max(1),
    }
}

pub(super) fn validate_path(path: &[String]) -> Result<(), String> {
    if path.len() > MAX_STREAM_PATH_DEPTH || path.iter().map(String::len).sum::<usize>() > MAX_STREAM_PATH_BYTES {
        return Err("stream paths exceed 64 levels / 4096 UTF-8 bytes".into());
    }
    Ok(())
}

pub(super) fn bounded_description(prefix: &str, text: &str, suffix: &str) -> String {
    let mut end = text.len().min(MAX_STREAM_PATH_BYTES - prefix.len() - suffix.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{prefix}{}{suffix}", &text[..end])
}
