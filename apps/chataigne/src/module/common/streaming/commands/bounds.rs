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
    if matches!(value, ParamValue::Reference(reference) if reference.relative_path_from_root().len() > MAX_COMMAND_VALUES)
    {
        return usize::MAX;
    }
    value.retained_bytes()
}
pub(super) fn source(text: &str) -> Result<(), String> {
    if text.len() > MAX_SOURCE_BYTES {
        Err("Stream command source exceeds 4 MiB".into())
    } else {
        Ok(())
    }
}

pub(crate) use crate::app::module::common::json::retained_weight as json_weight;

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
