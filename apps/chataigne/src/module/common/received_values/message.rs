use super::ReceivedValuePayload;
use golden_core::parameter::ParamValue;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReceivedValueMessage {
    pub path_segments: Vec<String>,
    pub payload: ReceivedValuePayload,
    pub source_description: String,
}

impl ReceivedValueMessage {
    /// Retained heap storage; the owning container accounts for the message itself.
    pub(crate) fn retained_weight(&self) -> usize {
        self.path_segments.capacity() * std::mem::size_of::<String>()
            + self.path_segments.iter().map(String::capacity).sum::<usize>()
            + self.source_description.capacity()
            + self.payload.retained_weight()
    }
}

impl ReceivedValuePayload {
    pub(crate) fn retained_weight(&self) -> usize {
        match self {
            Self::Single(value) => value_dynamic_weight(value),
            Self::Multi(values) => {
                values.capacity() * std::mem::size_of::<ParamValue>()
                    + values.iter().map(value_dynamic_weight).sum::<usize>()
            }
        }
    }
}

pub(super) fn value_dynamic_weight(value: &ParamValue) -> usize {
    match value {
        ParamValue::Str(value) | ParamValue::Enum(value) | ParamValue::File(value) => value.capacity(),
        ParamValue::Reference(value) => {
            value.cached_name.as_ref().map_or(0, String::capacity)
                + value.relative_path_from_root.capacity() * std::mem::size_of::<String>()
                + value
                    .relative_path_from_root
                    .iter()
                    .map(String::capacity)
                    .sum::<usize>()
        }
        _ => 0,
    }
}
