use golden_core::parameter::{ParamValue, ParameterEnumOption};

use super::{AUTOMATIC_CONFIGURATION, NO_AUDIO_DEVICE, SYSTEM_DEFAULT_DEVICE};

pub(crate) const NO_AUDIO_DRIVER: &str = "none";

pub(crate) fn default_audio_driver() -> String {
    #[cfg(test)]
    {
        "null".to_owned()
    }
    #[cfg(not(test))]
    {
        golden_audio::compiled_cpal_backend_catalog()
            .into_iter()
            .find(|backend| backend.is_platform_default)
            .map(|backend| backend.id.to_string())
            .unwrap_or_else(|| NO_AUDIO_DRIVER.to_owned())
    }
}

pub(crate) fn backend_options() -> Vec<ParameterEnumOption> {
    let mut options = vec![enum_option(NO_AUDIO_DRIVER, "None", 0)];
    #[cfg(test)]
    options.push(enum_option("null", "Test Audio", 1));
    #[cfg(not(test))]
    options.extend(
        golden_audio::compiled_cpal_backend_catalog()
            .into_iter()
            .enumerate()
            .map(|(index, backend)| {
                enum_option(
                    backend.id.as_str(),
                    backend.label.as_str(),
                    i32::try_from(index).unwrap_or(i32::MAX).saturating_add(1),
                )
            }),
    );
    options
}

pub(crate) fn input_device_options() -> Vec<ParameterEnumOption> {
    vec![enum_option(NO_AUDIO_DEVICE, "None", 0)]
}

pub(crate) fn device_options() -> Vec<ParameterEnumOption> {
    vec![enum_option(NO_AUDIO_DEVICE, "None", 0)]
}

pub(crate) fn output_device_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(NO_AUDIO_DEVICE, "None", 0),
        enum_option(SYSTEM_DEFAULT_DEVICE, "System Default", 1),
    ]
}

pub(crate) fn sample_rate_options() -> Vec<ParameterEnumOption> {
    vec![enum_option(AUTOMATIC_CONFIGURATION, "Automatic", 0)]
}

pub(crate) fn buffer_size_options() -> Vec<ParameterEnumOption> {
    vec![enum_option(AUTOMATIC_CONFIGURATION, "Automatic", 0)]
}

pub(super) fn enum_option(value: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: value.to_owned(),
        value: ParamValue::Enum(value.to_owned()),
        label: label.to_owned(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}
