use golden_io::{PayloadPool, PayloadReservation, RetainedPayload};
use golden_parameters::ParamValue;
use serde_json::Value;
use std::{mem::size_of, sync::Mutex};

pub(crate) const MAX_VALUES: usize = 4096;
pub(crate) const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_LABEL_BYTES: usize = 1024;

pub(crate) fn reserve(weight: usize) -> Result<PayloadReservation, &'static str> {
    static POOL: Mutex<Option<PayloadPool>> = Mutex::new(None);
    let mut pool = POOL.lock().unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        // History consumers may retain earlier events while another callback is staged.
        // Bytes remain the primary admission limit; small events need separate item headroom.
        *pool = Some(
            PayloadPool::new("script-host-input", 16_384, 64 * 1024 * 1024)
                .map_err(|_| "script input disposer unavailable")?,
        );
    }
    pool.as_ref()
        .expect("initialized script input pool")
        .try_reserve(weight)
        .map_err(|_| "script input lifetime capacity exhausted")
}

pub(crate) fn param_weight(value: &ParamValue) -> Result<usize, &'static str> {
    if let ParamValue::Reference(reference) = value
        && reference.relative_path_from_root.len() > MAX_VALUES
    {
        return Err("script reference exceeds 4096 path segments");
    }
    let weight = value.retained_bytes();
    if weight > MAX_INPUT_BYTES {
        return Err("script value exceeds 4 MiB retained storage");
    }
    Ok(weight)
}

pub(crate) fn json_weight(value: &Value) -> Result<usize, &'static str> {
    json_weight_with_limits(value, MAX_INPUT_BYTES, MAX_VALUES + 1, 32)
}

pub(crate) fn json_weight_with_limits(
    value: &Value,
    maximum_bytes: usize,
    maximum_values: usize,
    maximum_depth: usize,
) -> Result<usize, &'static str> {
    let mut pending = vec![(value, 0_usize)];
    let mut visited = 0;
    let mut weight = size_of::<Value>() + 64;
    while let Some((value, depth)) = pending.pop() {
        visited += 1;
        if visited > maximum_values || depth > maximum_depth {
            return Err("script event JSON exceeds its item/depth limit");
        }
        match value {
            Value::String(text) => weight = weight.saturating_add(text.capacity()),
            Value::Array(values) => {
                weight = weight.saturating_add(values.capacity().saturating_mul(size_of::<Value>()));
                if values.len().saturating_add(visited).saturating_add(pending.len()) > maximum_values
                    || weight > maximum_bytes
                {
                    return Err("script event JSON exceeds its item/storage limit");
                }
                pending.extend(values.iter().map(|value| (value, depth + 1)));
            }
            Value::Object(values) => {
                weight = weight.saturating_add(values.len().saturating_mul(256));
                if values.len().saturating_add(visited).saturating_add(pending.len()) > maximum_values
                    || weight > maximum_bytes
                {
                    return Err("script event JSON exceeds its item/storage limit");
                }
                for (key, value) in values {
                    weight = weight.saturating_add(key.capacity());
                    pending.push((value, depth + 1));
                }
            }
            _ => {}
        }
        if weight > maximum_bytes {
            return Err("script JSON exceeds its retained storage limit");
        }
    }
    Ok(weight)
}

/// Normalize object storage after shape validation, before sharing with consumers.
/// serde_json hides spare map capacity when its preserve_order feature is enabled.
pub(crate) fn canonicalize_objects(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(canonicalize_objects),
        Value::Object(values) => {
            for (key, mut value) in std::mem::take(values) {
                canonicalize_objects(&mut value);
                values.insert(key, value);
            }
        }
        _ => {}
    }
}

/// Rejected data remains owned by its preparing producer.
pub struct ScriptInputError<T> {
    input: Box<T>,
    pub(crate) message: &'static str,
}

impl<T> ScriptInputError<T> {
    pub(crate) fn new(input: T, message: &'static str) -> Self {
        Self {
            input: Box::new(input),
            message,
        }
    }
    /// Recover the original producer allocations after admission failure.
    pub fn into_input(self) -> T {
        *self.input
    }
}
impl<T> std::fmt::Display for ScriptInputError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl<T> std::fmt::Debug for ScriptInputError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl<T> std::error::Error for ScriptInputError<T> {}

pub(crate) fn retain_log(message: String) -> Result<RetainedPayload<String>, ScriptInputError<String>> {
    let admission = || {
        if message.capacity() > 64 * 1024 {
            return Err("script log exceeds 64 KiB retained storage");
        }
        let weight = size_of::<String>() + 64 + message.capacity();
        Ok((reserve(weight)?, weight))
    };
    match admission() {
        Ok((permit, weight)) => Ok(permit.retain(message, weight).expect("log fits its reservation")),
        Err(error) => Err(ScriptInputError::new(message, error)),
    }
}

#[cfg(test)]
mod tests;
