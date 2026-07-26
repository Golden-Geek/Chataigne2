use golden_core::parameter::{ParamValue, ParameterEnumOption};

pub(crate) fn backend_options() -> Vec<ParameterEnumOption> {
    vec![enum_option("platform_default", "Platform Default", 0)]
}

pub(crate) fn recovery_policy_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("wait_for_selected", "Wait for Selected", 0),
        enum_option("follow_system_default", "Follow System Default", 1),
    ]
}

pub(crate) fn buffer_policy_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("automatic", "Automatic", 0),
        enum_option("fixed", "Fixed", 1),
    ]
}

pub(crate) fn spectrum_window_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("hann", "Hann", 0),
        enum_option("blackman_harris", "Blackman-Harris", 1),
    ]
}

pub(crate) fn spectrum_overlap_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("none", "None", 0),
        enum_option("half", "50%", 1),
        enum_option("three_quarters", "75%", 2),
    ]
}

pub(crate) fn spectrum_spacing_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("linear", "Linear", 0),
        enum_option("logarithmic", "Logarithmic", 1),
    ]
}

pub(super) fn device_options(value: &str, label: &str) -> Vec<ParameterEnumOption> {
    vec![enum_option(value, label, 0)]
}

pub(super) fn enum_option(value: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: value.to_string(),
        value: ParamValue::Enum(value.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}
