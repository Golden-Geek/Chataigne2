use golden_core::parameter::ParamValue;
use std::io::{self, Write};

pub(super) const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_COMMAND_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_COMMAND_VALUES: usize = 4096;

pub(crate) fn validate_values(values: &[ParamValue]) -> Result<(), String> {
    if values.len() > MAX_COMMAND_VALUES {
        return Err("Stream command exceeds 4096 values".into());
    }
    let mut weight = 0_usize;
    for value in values {
        weight = weight.saturating_add(value_weight(value));
        if weight > MAX_SOURCE_BYTES {
            return Err("Stream command source exceeds 4 MiB retained storage".into());
        }
    }
    Ok(())
}
pub(crate) fn value_weight(value: &ParamValue) -> usize {
    std::mem::size_of::<ParamValue>()
        + match value {
            ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) => text.capacity(),
            ParamValue::Reference(reference) => {
                let path = reference.relative_path_from_root();
                if path.len() > MAX_COMMAND_VALUES {
                    return usize::MAX;
                }
                reference.cached_name.as_ref().map_or(0, String::capacity)
                    + reference.relative_path_from_root.capacity() * std::mem::size_of::<String>()
                    + path.iter().map(String::capacity).sum::<usize>()
            }
            _ => 0,
        }
}
pub(super) fn source(text: &str) -> Result<(), String> {
    if text.len() > MAX_SOURCE_BYTES {
        Err("Stream command source exceeds 4 MiB".into())
    } else {
        Ok(())
    }
}

/// A conservative allocation budget checked before any JSON clone or typed
/// decode. Arrays include spare capacity; maps include per-entry tree overhead.
pub(crate) fn json_weight(
    value: &serde_json::Value,
    bytes: usize,
    items: usize,
    depth: usize,
) -> Result<usize, String> {
    use serde_json::Value;
    let mut pending = vec![(value, 0_usize)];
    let mut visited = 0_usize;
    let mut weight = std::mem::size_of::<Value>();
    while let Some((value, level)) = pending.pop() {
        visited += 1;
        if visited > items || level > depth {
            return Err("Stream JSON exceeds its item/depth admission limit".into());
        }
        match value {
            Value::String(text) => weight = weight.saturating_add(text.capacity()),
            Value::Array(values) => {
                weight = weight.saturating_add(values.capacity().saturating_mul(std::mem::size_of::<Value>()));
                if weight > bytes || values.len().saturating_add(visited).saturating_add(pending.len()) > items {
                    return Err("Stream JSON exceeds its retained-byte/item admission limit".into());
                }
                pending.extend(values.iter().map(|value| (value, level + 1)));
            }
            Value::Object(values) => {
                weight = weight.saturating_add(values.len().saturating_mul(256));
                if weight > bytes || values.len().saturating_add(visited).saturating_add(pending.len()) > items {
                    return Err("Stream JSON exceeds its retained-byte/item admission limit".into());
                }
                for (key, value) in values {
                    weight = weight.saturating_add(key.capacity());
                    pending.push((value, level + 1));
                }
            }
            _ => {}
        }
        if weight > bytes {
            return Err("Stream JSON exceeds its retained-byte admission limit".into());
        }
    }
    Ok(weight)
}

#[derive(Default)]
pub(super) struct Output(pub Vec<u8>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self
            .0
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= MAX_COMMAND_BYTES)
            .ok_or_else(|| io::Error::other("Stream command output exceeds 1 MiB"))?;
        if self.0.capacity() < length {
            self.0.reserve_exact(length - self.0.len());
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
