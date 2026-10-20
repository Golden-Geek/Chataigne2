use super::super::super::super::{runtime, settings::enum_option, NO_AUDIO_DEVICE, SYSTEM_DEFAULT_DEVICE};
use golden_core::parameter::{ParamValue, ParameterEnumOption};
use std::io::Write;

pub(super) const MAX_OPTIONS_BYTES: usize = 32 * 1024 * 1024;
const MAX_OPTIONS: usize = 32_768;
const MAX_ID_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceChoice {
    Input,
    Output,
    Duplex,
}

impl DeviceChoice {
    fn supports(self, descriptor: &golden_audio::AudioDeviceDescriptor) -> bool {
        match self {
            Self::Input => descriptor.supports(golden_audio::AudioDirection::Input),
            Self::Output => descriptor.supports(golden_audio::AudioDirection::Output),
            Self::Duplex => {
                descriptor.supports(golden_audio::AudioDirection::Input)
                    && descriptor.supports(golden_audio::AudioDirection::Output)
            }
        }
    }

    fn direction(self) -> Option<golden_audio::AudioDirection> {
        match self {
            Self::Input => Some(golden_audio::AudioDirection::Input),
            Self::Output => Some(golden_audio::AudioDirection::Output),
            Self::Duplex => None,
        }
    }
}

pub(crate) fn device_options_for_current(
    current: &str,
    state: &golden_audio::AudioDeviceInspectorState,
    choice: DeviceChoice,
    driver: Option<&golden_audio::BackendId>,
) -> Result<Vec<ParameterEnumOption>, &'static str> {
    let mut options = Options::default();
    options.push(enum_option(NO_AUDIO_DEVICE, "None", 0))?;
    let mut descriptors = std::collections::HashMap::new();
    for device in &state.devices {
        let entry = descriptors.entry(&device.target).or_insert(None);
        if entry.is_none() && choice.supports(device) {
            *entry = Some(device);
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut option_targets = Vec::<(String, golden_audio::AudioDeviceTargetId)>::new();
    if driver.is_some_and(runtime::supports_system_default) {
        options.push(enum_option(SYSTEM_DEFAULT_DEVICE, "System Default", 10))?;
    }
    for (index, catalog_entry) in state
        .device_catalog
        .iter()
        .filter(|device| driver.is_some_and(|driver| device.target.backend() == driver))
        .enumerate()
    {
        if !seen.insert(&catalog_entry.target) {
            continue;
        }
        let descriptor = match descriptors.get(&catalog_entry.target) {
            Some(Some(device)) => Some(*device),
            Some(None) => continue,
            None => None,
        };
        let selection = descriptor.map_or_else(
            || golden_audio::AudioDeviceSelection::from_catalog_entry(catalog_entry),
            golden_audio::AudioDeviceSelection::from_descriptor,
        );
        let variant_id = selection_value(&selection)?;
        options.push(enum_option(
            variant_id.as_str(),
            catalog_entry.label.as_str(),
            100_i32.saturating_add(i32::try_from(index).unwrap_or(i32::MAX)),
        ))?;
        option_targets.push((variant_id, catalog_entry.target.clone()));
    }
    options.sort_by(|left, right| {
        left.ordering
            .cmp(&right.ordering)
            .then_with(|| left.label.cmp(&right.label))
    });
    options.dedup_by(|left, right| left.variant_id == right.variant_id);

    let mut authored_selection = None;
    if !options.iter().any(|option| option.variant_id == current) {
        if let Ok(authored) = serde_json::from_str::<golden_audio::AudioDeviceSelection>(current) {
            let recovered = choice
                .direction()
                .and_then(|direction| {
                    match golden_audio::match_device_selection(&authored, direction, state.devices.as_slice()) {
                        golden_audio::AudioDeviceMatch::Matched(device) => {
                            Some((device.target.clone(), device.label.clone()))
                        }
                        golden_audio::AudioDeviceMatch::Missing | golden_audio::AudioDeviceMatch::Ambiguous(_) => None,
                    }
                })
                .or_else(|| {
                    state
                        .devices
                        .iter()
                        .find(|device| device.target == authored.target && choice.supports(device))
                        .map(|device| (device.target.clone(), device.label.clone()))
                })
                .or_else(|| {
                    state
                        .device_catalog
                        .iter()
                        .find(|entry| entry.target == authored.target)
                        .map(|entry| (entry.target.clone(), entry.label.clone()))
                });
            if let Some((target, label)) = recovered {
                if let Some((variant, _)) = option_targets.iter().find(|(_, candidate)| *candidate == target) {
                    if let Some(option) = options.iter_mut().find(|option| option.variant_id == *variant) {
                        // Keep the authored serialized identity while
                        // refreshing the user-facing live label.
                        option.variant_id = current.to_owned();
                        option.value = ParamValue::Enum(current.to_owned());
                        option.label = label;
                    }
                }
            }
            authored_selection = Some(authored);
        }
    }
    if !options.iter().any(|option| option.variant_id == current) {
        let label = authored_selection
            .as_ref()
            .map(|selection| selection.last_known_label.trim())
            .filter(|label| !label.is_empty())
            .unwrap_or("Unavailable device");
        let mut missing = enum_option(current, format!("Missing: {label}").as_str(), i32::MAX);
        missing.tags.push("missing".to_owned());
        options.push(missing)?;
    }
    if options_storage(&options.values) > MAX_OPTIONS_BYTES {
        return Err("Sound Card enum domain exceeds 32 MiB");
    }
    Ok(options.values)
}

struct Options {
    values: Vec<ParameterEnumOption>,
    bytes: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            // Include the small initial Vec capacity as well as the shared
            // allocation headers before admitting any option payload.
            bytes: size_of::<golden_core::parameter::ParameterEnumDomain>()
                + 2 * size_of::<usize>()
                + 4 * size_of::<ParameterEnumOption>(),
        }
    }
}
impl std::ops::Deref for Options {
    type Target = Vec<ParameterEnumOption>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl std::ops::DerefMut for Options {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}
impl Options {
    fn push(&mut self, option: ParameterEnumOption) -> Result<(), &'static str> {
        let weight = option.retained_heap_bytes() + 2 * size_of::<ParameterEnumOption>() + 2 * size_of::<usize>();
        if self.values.len() >= MAX_OPTIONS || self.bytes.saturating_add(weight) > MAX_OPTIONS_BYTES {
            return Err("Sound Card enum domain exceeds 32768 options / 32 MiB");
        }
        self.bytes += weight;
        self.values.push(option);
        Ok(())
    }
}
pub(super) fn options_storage(options: &Vec<ParameterEnumOption>) -> usize {
    golden_core::parameter::ParameterEnumDomain::retained_bytes_for(options)
}
fn selection_value(selection: &golden_audio::AudioDeviceSelection) -> Result<String, &'static str> {
    struct Limited(Vec<u8>);
    impl Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_ID_BYTES {
                return Err(std::io::Error::other("device identity exceeds 1 MiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Limited(Vec::new());
    serde_json::to_writer(&mut output, selection).map_err(|_| "Sound Card device identity exceeds 1 MiB")?;
    String::from_utf8(output.0).map_err(|_| "Sound Card device identity is not UTF-8")
}

pub(super) fn numeric_options(
    current: &str,
    unit: &str,
    values: &[u32],
) -> Result<Vec<ParameterEnumOption>, &'static str> {
    use super::super::super::super::AUTOMATIC_CONFIGURATION;
    let mut options = Options::default();
    options.push(enum_option(AUTOMATIC_CONFIGURATION, "Automatic", 0))?;
    for (index, value) in values.iter().enumerate() {
        options.push(enum_option(
            &value.to_string(),
            &format!("{value} {unit}"),
            i32::try_from(index).unwrap_or(i32::MAX).saturating_add(1),
        ))?;
    }
    if !options.iter().any(|option| option.variant_id == current) {
        let mut missing = enum_option(current, &format!("Unavailable: {current} {unit}"), i32::MAX);
        missing.tags.push("missing".into());
        options.push(missing)?;
    }
    Ok(options.values)
}

#[cfg(test)]
mod tests;
