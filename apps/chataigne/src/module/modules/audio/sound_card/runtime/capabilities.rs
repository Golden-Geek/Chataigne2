use super::*;

const COMMON_SAMPLE_RATES: &[u32] = &[
    8_000, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000,
];
const COMMON_BUFFER_SIZES: &[u32] = &[16, 32, 64, 128, 256, 512, 1_024, 2_048, 4_096, 8_192];

pub(crate) fn supports_system_default(driver: &BackendId) -> bool {
    if driver == &NullBackend::backend_id() {
        return true;
    }
    golden_audio::compiled_cpal_backend_catalog()
        .iter()
        .find(|backend| backend.id == *driver)
        .is_some_and(|backend| backend.supports_system_default)
}

pub(crate) fn configuration_capabilities(
    driver: Option<&BackendId>,
    inspector: &AudioDeviceInspectorState,
    input_value: &str,
    output_value: &str,
    sample_rate_value: &str,
    buffer_size_value: &str,
) -> (Vec<u32>, Vec<u32>) {
    let input = resolve_selection(driver, inspector, input_value, AudioDirection::Input)
        .ok()
        .flatten();
    let output = resolve_selection(driver, inspector, output_value, AudioDirection::Output)
        .ok()
        .flatten();
    let input_device = input
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, AudioDirection::Input));
    let output_device = output
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, AudioDirection::Output));
    let rates = supported_sample_rates(input_device, output_device, parse_fixed_value(buffer_size_value));
    let rate = parse_fixed_value(sample_rate_value).or_else(|| preferred_rate(&rates));
    let buffers = supported_buffer_sizes(input_device, output_device, rate);
    (rates, buffers)
}

pub(crate) fn selected_devices_are_available(
    driver: Option<&BackendId>,
    inspector: &AudioDeviceInspectorState,
    input_value: &str,
    output_value: &str,
) -> bool {
    [
        (input_value, AudioDirection::Input),
        (output_value, AudioDirection::Output),
    ]
    .into_iter()
    .all(|(value, direction)| {
        resolve_selection(driver, inspector, value, direction)
            .ok()
            .is_some_and(|selection| {
                selection.is_none()
                    || selection
                        .as_ref()
                        .is_some_and(|selection| matched_device(inspector, selection, direction).is_some())
            })
    })
}

pub(crate) fn selected_probe_targets(
    driver: Option<&BackendId>,
    input_value: &str,
    output_value: &str,
) -> Result<Vec<AudioDeviceTargetId>, String> {
    let Some(driver) = driver else {
        return Ok(Vec::new());
    };
    let parse_target = |value: &str| -> Result<Option<AudioDeviceTargetId>, String> {
        if value == NO_AUDIO_DEVICE || value == SYSTEM_DEFAULT_DEVICE {
            return Ok(None);
        }
        let selection = serde_json::from_str::<AudioDeviceSelection>(value)
            .map_err(|error| format!("invalid audio device selection: {error}"))?;
        if selection.target.backend() != driver {
            return Err(
                "selected audio device belongs to another audio driver".to_owned(),
            );
        }
        Ok(Some(selection.target))
    };
    let input_target = parse_target(input_value)?;
    let output_target = parse_target(output_value)?;
    if driver.as_str() == "asio"
        && input_target.is_some()
        && output_target.is_some()
        && input_target != output_target
    {
        return Err("ASIO input and output must use the same driver device".to_owned());
    }
    let mut targets = Vec::with_capacity(2);
    for target in [input_target, output_target].into_iter().flatten() {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Ok(targets)
}

pub(crate) fn probe_targets_are_pending(
    inspector: &AudioDeviceInspectorState,
    targets: &[AudioDeviceTargetId],
) -> bool {
    targets.iter().any(|target| {
        inspector
            .device_catalog
            .iter()
            .any(|entry| entry.target == *target)
            && !inspector
                .devices
                .iter()
                .any(|device| device.target == *target)
    })
}

pub(crate) fn requires_disable_before_probe(
    driver: Option<&BackendId>,
    inspector: &AudioDeviceInspectorState,
    targets: &[AudioDeviceTargetId],
) -> bool {
    if driver.is_none_or(|driver| driver.as_str() != "asio") || targets.is_empty() {
        return false;
    }
    let active_targets = [
        inspector.input.active_target.as_ref(),
        inspector.output.active_target.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    targets
        .iter()
        .any(|target| active_targets.iter().any(|active| **active != *target))
}

pub(crate) fn selected_physical_channels(
    driver: Option<&BackendId>,
    inspector: &AudioDeviceInspectorState,
    value: &str,
    direction: AudioDirection,
) -> Vec<PhysicalChannelKey> {
    resolve_selection(driver, inspector, value, direction)
        .ok()
        .flatten()
        .as_ref()
        .and_then(|selection| matched_device(inspector, selection, direction))
        .map(|descriptor| match direction {
            AudioDirection::Input => descriptor.input_channels.as_slice(),
            AudioDirection::Output => descriptor.output_channels.as_slice(),
        })
        .unwrap_or_default()
        .iter()
        .map(|channel| channel.key.clone())
        .collect()
}

pub(crate) fn device_selection_value(selection: &AudioDeviceSelection) -> String {
    serde_json::to_string(selection).expect("audio device selections serialize")
}

pub(crate) fn supported_sample_rates(
    input: Option<&AudioDeviceDescriptor>,
    output: Option<&AudioDeviceDescriptor>,
    buffer_frames: Option<u32>,
) -> Vec<u32> {
    let mut candidates = COMMON_SAMPLE_RATES.iter().copied().collect::<BTreeSet<_>>();
    for (device, direction) in active_devices(input, output) {
        for configuration in matching_configurations(device, direction) {
            candidates.insert(configuration.min_sample_rate);
            candidates.insert(configuration.max_sample_rate);
        }
    }
    intersect_candidates(candidates, input, output, |configuration, value| {
        configuration.min_sample_rate <= value
            && value <= configuration.max_sample_rate
            && buffer_frames.is_none_or(|frames| {
                configuration.buffer_frames.min <= frames && frames <= configuration.buffer_frames.max
            })
    })
}

pub(crate) fn supported_buffer_sizes(
    input: Option<&AudioDeviceDescriptor>,
    output: Option<&AudioDeviceDescriptor>,
    sample_rate: Option<u32>,
) -> Vec<u32> {
    let mut candidates = COMMON_BUFFER_SIZES.iter().copied().collect::<BTreeSet<_>>();
    for (device, direction) in active_devices(input, output) {
        for configuration in matching_configurations(device, direction) {
            candidates.extend([
                configuration.buffer_frames.min,
                configuration.buffer_frames.max,
                configuration.buffer_frames.preferred,
            ]);
        }
    }
    intersect_candidates(candidates, input, output, |configuration, value| {
        configuration.buffer_frames.min <= value
            && value <= configuration.buffer_frames.max
            && sample_rate
                .is_none_or(|rate| configuration.min_sample_rate <= rate && rate <= configuration.max_sample_rate)
    })
}

fn intersect_candidates(
    candidates: BTreeSet<u32>,
    input: Option<&AudioDeviceDescriptor>,
    output: Option<&AudioDeviceDescriptor>,
    supports: impl Fn(&golden_audio::SupportedStreamConfiguration, u32) -> bool,
) -> Vec<u32> {
    if input.is_none() && output.is_none() {
        return Vec::new();
    }
    candidates
        .into_iter()
        .filter(|candidate| {
            input.is_none_or(|device| {
                matching_configurations(device, AudioDirection::Input)
                    .any(|configuration| supports(configuration, *candidate))
            }) && output.is_none_or(|device| {
                matching_configurations(device, AudioDirection::Output)
                    .any(|configuration| supports(configuration, *candidate))
            })
        })
        .collect()
}

fn active_devices<'a>(
    input: Option<&'a AudioDeviceDescriptor>,
    output: Option<&'a AudioDeviceDescriptor>,
) -> impl Iterator<Item = (&'a AudioDeviceDescriptor, AudioDirection)> {
    input
        .map(|device| (device, AudioDirection::Input))
        .into_iter()
        .chain(output.map(|device| (device, AudioDirection::Output)))
}

fn matching_configurations(
    device: &AudioDeviceDescriptor,
    direction: AudioDirection,
) -> impl Iterator<Item = &golden_audio::SupportedStreamConfiguration> {
    let channels = match direction {
        AudioDirection::Input => device.input_channels.len(),
        AudioDirection::Output => device.output_channels.len(),
    };
    device.supported_configurations.iter().filter(move |configuration| {
        configuration.direction == direction && usize::from(configuration.channels) == channels
    })
}
