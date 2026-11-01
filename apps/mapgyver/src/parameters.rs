//! Shared app parameter presentation; authored values remain Golden Parameter nodes.
use golden_core::parameter::{ParamValue, ParameterEnumOption};

pub(crate) mod reference_cycle;

pub(crate) fn enum_options(values: &[&str]) -> Vec<ParameterEnumOption> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| ParameterEnumOption {
            variant_id: (*value).into(),
            value: ParamValue::Enum((*value).into()),
            label: (*value).into(),
            tags: vec![],
            ordering: Some(index as i32),
        })
        .collect()
}
