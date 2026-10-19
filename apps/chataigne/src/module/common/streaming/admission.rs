use golden_io::{PayloadPool, RetainedPayload};
use std::{
    mem::size_of,
    ops::Deref,
    sync::{Mutex, OnceLock},
};

use golden_core::parameter::ParamValue;

use crate::app::module::common::received_values::{ReceivedValueMessage, ReceivedValuePayload};

pub(crate) const MAX_STREAM_MESSAGES: usize = 65_536;
pub(crate) const MAX_STREAM_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_STREAM_PATH_DEPTH: usize = 64;
pub(crate) const MAX_STREAM_PATH_BYTES: usize = 4096;
pub(super) const MAX_VALUE_UNITS: usize = MAX_STREAM_BYTES / size_of::<ParamValue>();
pub(super) const MAX_JSON_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default)]
pub(super) struct MessagePreparation {
    messages: Vec<ReceivedValueMessage>,
    dynamic_bytes: usize,
    value_units: usize,
}

impl MessagePreparation {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.messages
            .capacity()
            .saturating_mul(size_of::<ReceivedValueMessage>())
            .saturating_add(self.dynamic_bytes)
            .saturating_add(size_of::<Self>())
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
        let payload_budget = MAX_STREAM_BYTES - size_of::<Self>();
        let needed = self.messages.len() + 1;
        let mut capacity = self.messages.capacity();
        if needed > capacity {
            capacity = capacity.saturating_mul(2).max(needed).clamp(4, MAX_STREAM_MESSAGES);
        }
        if dynamic_bytes.saturating_add(capacity * size_of::<ReceivedValueMessage>()) > payload_budget {
            // A geometric reservation must not reject a prefix that fits with exact storage.
            capacity = needed.max(self.messages.capacity());
            if dynamic_bytes.saturating_add(capacity * size_of::<ReceivedValueMessage>()) > payload_budget {
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
        let prepared = MessagePreparation {
            messages,
            dynamic_bytes,
            value_units: units,
        };
        if units > MAX_VALUE_UNITS || prepared.retained_bytes() > MAX_STREAM_BYTES {
            return Err("stream expansion exceeds 16 MiB of retained storage".into());
        }
        prepared.finish()
    }
}

/// Immutable admitted input. The final message/vector/string destruction belongs to the
/// shared bounded disposer, even if a node is replaced or its producer panics.
#[derive(Clone, Debug, Default)]
pub(crate) struct PreparedMessages(Option<RetainedPayload<MessagePreparation>>);

impl PreparedMessages {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.0.as_ref().map_or(0, |data| data.retained_bytes())
    }
}

impl Deref for PreparedMessages {
    type Target = [ReceivedValueMessage];
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().map_or(&[], |data| data.messages.as_slice())
    }
}

impl MessagePreparation {
    pub(super) fn finish(self) -> Result<PreparedMessages, String> {
        if self.messages.is_empty() {
            return Ok(PreparedMessages::default());
        }
        let weight = self.retained_bytes();
        retain_payload(self, weight).map(|value| PreparedMessages(Some(value)))
    }
}

pub(crate) fn retain_payload<T: Send + Sync + 'static>(value: T, weight: usize) -> Result<RetainedPayload<T>, String> {
    payload_pool()?
        .try_retain(value, weight)
        .map_err(|_| "stream payload lifetime capacity exhausted; batch was not admitted".into())
}

fn payload_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("chataigne-stream-payloads", 256, 256 * 1024 * 1024)
                .map_err(|error| format!("failed to start stream payload disposer: {error}"))?,
        );
    }
    Ok(pool.as_ref().expect("installed payload disposer").clone())
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
