use std::collections::BTreeMap;

use cpal::{
    Device, Host, HostId, SampleFormat, SupportedBufferSize, SupportedStreamConfigRange,
    traits::{DeviceTrait, HostTrait},
};

use crate::{
    AudioDeviceCatalogEntry, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceId, AudioDeviceInventory,
    AudioDeviceTargetId, AudioDirection, AudioError, AudioSampleFormat, BackendId, PhysicalChannelDescriptor,
    PhysicalChannelKey, SupportedBufferFrames, SupportedStreamConfiguration, profile_key_for,
};

use super::{backend_id_for_host, error::map_cpal_error};

pub(super) fn discover_host_inventory(host_id: HostId, host: &Host) -> Result<AudioDeviceInventory, AudioError> {
    let default_input = default_id(host.default_input_device());
    let default_output = default_id(host.default_output_device());
    let devices = host
        .devices()
        .map_err(|error| map_cpal_error(error, "enumerate audio devices", Some(backend_id_for_host(host_id))))?;
    let mut inventory = AudioDeviceInventory::default();
    for device in devices {
        match descriptor_for_device_with_defaults(host_id, &device, default_input.as_deref(), default_output.as_deref())
        {
            Ok(descriptor) => {
                inventory.catalog.push(AudioDeviceCatalogEntry::from(&descriptor));
                inventory.devices.push(descriptor);
            }
            Err(_) => {
                if let Some(entry) = catalog_entry_for_device(host_id, &device) {
                    inventory.catalog.push(entry);
                }
            }
        }
    }
    Ok(inventory)
}

fn catalog_entry_for_device(host_id: HostId, device: &Device) -> Option<AudioDeviceCatalogEntry> {
    let target = AudioDeviceTargetId::Device {
        backend: backend_id_for_host(host_id),
        device: AudioDeviceId::new(device.id().ok()?.to_string()).ok()?,
    };
    let label = device
        .description()
        .ok()
        .map_or_else(|| device.to_string(), |value| value.name().to_owned());
    Some(AudioDeviceCatalogEntry { target, label })
}

pub(super) fn descriptor_for_device(
    host_id: HostId,
    host: &Host,
    device: &Device,
) -> Result<AudioDeviceDescriptor, AudioError> {
    descriptor_for_device_with_defaults(
        host_id,
        device,
        default_id(host.default_input_device()).as_deref(),
        default_id(host.default_output_device()).as_deref(),
    )
}

/// Describes an exact device without asking the host for system defaults.
///
/// ASIO has no system-default concept, and CPAL implements its default lookup by
/// enumerating drivers. Exact ASIO probes must use this path so selecting one
/// driver never initializes the others.
pub(super) fn descriptor_for_exact_device(
    host_id: HostId,
    device: &Device,
) -> Result<AudioDeviceDescriptor, AudioError> {
    descriptor_for_device_with_defaults(host_id, device, None, None)
}

fn descriptor_for_device_with_defaults(
    host_id: HostId,
    device: &Device,
    default_input: Option<&str>,
    default_output: Option<&str>,
) -> Result<AudioDeviceDescriptor, AudioError> {
    let backend = backend_id_for_host(host_id);
    let description = device.description().ok();
    let label = description
        .as_ref()
        .map_or_else(|| device.to_string(), |value| value.name().to_owned());
    let input_configurations = configurations(device, AudioDirection::Input, backend.clone())?;
    let output_configurations = configurations(device, AudioDirection::Output, backend.clone())?;
    let input_count = max_channels(&input_configurations);
    let output_count = max_channels(&output_configurations);
    let native_id = device.id().ok().map(|id| id.to_string());
    let mut properties = BTreeMap::new();
    if let Some(description) = &description {
        if let Some(driver) = description.driver() {
            properties.insert("driver".to_owned(), driver.to_owned());
        }
        properties.insert("device_type".to_owned(), description.device_type().to_string());
        properties.insert("direction".to_owned(), description.direction().to_string());
        for (index, line) in description.extended().enumerate() {
            properties.insert(format!("detail_{index}"), line.to_owned());
        }
    }
    let fingerprint = AudioDeviceFingerprint {
        vendor: description
            .as_ref()
            .and_then(|description| description.manufacturer().map(str::to_owned)),
        product: Some(label.clone()),
        serial: None,
        transport: description
            .as_ref()
            .map(|description| description.interface_type().to_string())
            .filter(|value| value != "Unknown"),
        backend_path: description
            .as_ref()
            .and_then(|description| description.address().map(str::to_owned)),
        input_channels: input_count,
        output_channels: output_count,
        properties,
    };
    let stable_id = native_id.is_some();
    let device_id = native_id.clone().unwrap_or_else(|| {
        let provisional_target = AudioDeviceTargetId::Device {
            backend: backend.clone(),
            device: AudioDeviceId::from_static("unidentified"),
        };
        profile_key_for(&provisional_target, Some(&fingerprint), None).into_inner()
    });
    let target = AudioDeviceTargetId::Device {
        backend,
        device: AudioDeviceId::new(device_id.clone()).expect("CPAL device identifiers are non-empty and trimmed"),
    };
    let profile_key = profile_key_for(&target, (!stable_id).then_some(&fingerprint), None);
    let mut supported_configurations = input_configurations;
    supported_configurations.extend(output_configurations);
    Ok(AudioDeviceDescriptor {
        target,
        label,
        stable_id,
        fingerprint,
        profile_key,
        input_channels: physical_channels("input", input_count),
        output_channels: physical_channels("output", output_count),
        supported_configurations,
        is_system_default_input: native_id.as_deref() == default_input,
        is_system_default_output: native_id.as_deref() == default_output,
    })
}

fn configurations(
    device: &Device,
    direction: AudioDirection,
    backend: BackendId,
) -> Result<Vec<SupportedStreamConfiguration>, AudioError> {
    match direction {
        AudioDirection::Input => configurations_from_ranges(device.supported_input_configs(), direction, backend),
        AudioDirection::Output => configurations_from_ranges(device.supported_output_configs(), direction, backend),
    }
}

fn configurations_from_ranges(
    ranges: Result<impl Iterator<Item = SupportedStreamConfigRange>, cpal::Error>,
    direction: AudioDirection,
    backend: BackendId,
) -> Result<Vec<SupportedStreamConfiguration>, AudioError> {
    match ranges {
        Ok(ranges) => Ok(ranges
            .filter_map(|range| supported_configuration(direction, range))
            .collect()),
        Err(error) if matches!(error.kind(), cpal::ErrorKind::UnsupportedOperation) => Ok(Vec::new()),
        Err(error) => Err(map_cpal_error(error, "query device formats", Some(backend))),
    }
}

fn supported_configuration(
    direction: AudioDirection,
    range: SupportedStreamConfigRange,
) -> Option<SupportedStreamConfiguration> {
    let sample_format = sample_format_from_cpal(range.sample_format())?;
    let buffer_frames = match range.buffer_size() {
        SupportedBufferSize::Range { min, max } => SupportedBufferFrames {
            min: *min,
            max: *max,
            preferred: 128_u32.clamp(*min, *max),
        },
        SupportedBufferSize::Unknown => SupportedBufferFrames {
            min: 1,
            max: 65_536,
            // The native host chooses the actual callback size. Keep enough
            // render-bridge headroom for common high-latency/Bluetooth paths.
            preferred: 2_048,
        },
    };
    Some(SupportedStreamConfiguration {
        direction,
        channels: range.channels(),
        sample_format,
        min_sample_rate: range.min_sample_rate(),
        max_sample_rate: range.max_sample_rate(),
        buffer_frames,
    })
}

pub(super) fn sample_format_from_cpal(format: SampleFormat) -> Option<AudioSampleFormat> {
    match format {
        SampleFormat::I8 => Some(AudioSampleFormat::I8),
        SampleFormat::I16 => Some(AudioSampleFormat::I16),
        SampleFormat::I24 => Some(AudioSampleFormat::I24),
        SampleFormat::I32 => Some(AudioSampleFormat::I32),
        SampleFormat::I64 => Some(AudioSampleFormat::I64),
        SampleFormat::U8 => Some(AudioSampleFormat::U8),
        SampleFormat::U16 => Some(AudioSampleFormat::U16),
        SampleFormat::U24 => Some(AudioSampleFormat::U24),
        SampleFormat::U32 => Some(AudioSampleFormat::U32),
        SampleFormat::U64 => Some(AudioSampleFormat::U64),
        SampleFormat::F32 => Some(AudioSampleFormat::F32),
        SampleFormat::F64 => Some(AudioSampleFormat::F64),
        SampleFormat::DsdU8 | SampleFormat::DsdU16 | SampleFormat::DsdU32 => None,
        _ => None,
    }
}

pub(super) fn sample_format_to_cpal(format: AudioSampleFormat) -> Option<SampleFormat> {
    match format {
        AudioSampleFormat::I8 => Some(SampleFormat::I8),
        AudioSampleFormat::I16 => Some(SampleFormat::I16),
        AudioSampleFormat::I24 => Some(SampleFormat::I24),
        AudioSampleFormat::I32 => Some(SampleFormat::I32),
        AudioSampleFormat::I64 => Some(SampleFormat::I64),
        AudioSampleFormat::U8 => Some(SampleFormat::U8),
        AudioSampleFormat::U16 => Some(SampleFormat::U16),
        AudioSampleFormat::U24 => Some(SampleFormat::U24),
        AudioSampleFormat::U32 => Some(SampleFormat::U32),
        AudioSampleFormat::U64 => Some(SampleFormat::U64),
        AudioSampleFormat::F32 => Some(SampleFormat::F32),
        AudioSampleFormat::F64 => Some(SampleFormat::F64),
    }
}

fn default_id(device: Option<Device>) -> Option<String> {
    device.and_then(|device| device.id().ok()).map(|id| id.to_string())
}

fn max_channels(configurations: &[SupportedStreamConfiguration]) -> u16 {
    configurations
        .iter()
        .map(|configuration| configuration.channels)
        .max()
        .unwrap_or(0)
}

fn physical_channels(prefix: &str, count: u16) -> Vec<PhysicalChannelDescriptor> {
    (0..count)
        .map(|index| PhysicalChannelDescriptor {
            key: PhysicalChannelKey::new(format!("{prefix}:{index}"))
                .expect("generated physical channel keys are non-empty"),
            label: format!("{} {}", if prefix == "input" { "Input" } else { "Output" }, index + 1),
            position: None,
        })
        .collect()
}
