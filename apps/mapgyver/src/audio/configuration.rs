//! Resolve authored keys against the current device inventory without rewriting preferences.
use super::preferences::{ChoiceEdit, DEFAULT, Settings, UNASSIGNED, option};
use golden_audio::{
    AudioChannelId, AudioConfiguration, AudioDeviceCatalogEntry, AudioDeviceDescriptor, AudioDeviceId,
    AudioDeviceInspectorState, AudioDeviceMatch, AudioDeviceSelection, AudioDeviceTargetId, AudioDirection,
    AudioRecoveryPolicy, AudioRouteId, BackendId, DirectionConfiguration, GainDb, OutputPatchRoute, PlaybackRoute,
    VirtualOutputChannel, match_device_selection,
};
use std::collections::HashSet;

fn device_token(target: &AudioDeviceTargetId) -> String {
    match target {
        AudioDeviceTargetId::Device { backend, device } => format!("{backend}/{device}"),
        AudioDeviceTargetId::SystemDefault { .. } => DEFAULT.into(),
    }
}

fn selected_device(
    value: &str,
    backend: Option<&BackendId>,
    devices: &[AudioDeviceDescriptor],
) -> Result<AudioDeviceSelection, String> {
    if value == DEFAULT {
        let backend = backend.cloned().ok_or("No native audio backend is available")?;
        return Ok(AudioDeviceSelection::follow_system_default(
            backend,
            AudioDirection::Output,
        ));
    }
    let (backend, device) = value.split_once('/').ok_or("Invalid output device identity")?;
    let target = AudioDeviceTargetId::Device {
        backend: BackendId::new(backend).map_err(|error| error.to_string())?,
        device: AudioDeviceId::new(device).map_err(|error| error.to_string())?,
    };
    Ok(devices.iter().find(|device| device.target == target).map_or_else(
        || {
            AudioDeviceSelection::from_catalog_entry(&AudioDeviceCatalogEntry {
                target: target.clone(),
                label: value.into(),
            })
        },
        AudioDeviceSelection::from_descriptor,
    ))
}

pub(super) fn resolve(
    settings: &Settings,
    backend: Option<&BackendId>,
    inventory: &AudioDeviceInspectorState,
) -> Result<(AudioConfiguration, Vec<ChoiceEdit>), String> {
    let selection = selected_device(&settings.device.value, backend, &inventory.devices)?;
    let matched = match_device_selection(&selection, AudioDirection::Output, &inventory.devices);
    let device = match &matched {
        AudioDeviceMatch::Matched(device) => Some(device.as_ref()),
        _ => None,
    };
    let channels = device.map_or(&[][..], |device| device.output_channels.as_slice());
    let mut configuration = AudioConfiguration::empty();
    configuration.output = DirectionConfiguration {
        // Keep discovery pointed at the selected device while retiring the previous stream.
        // Golden Audio can only enable a render direction with a concrete physical layout.
        enabled: settings.enabled && !channels.is_empty(),
        recovery_policy: if settings.device.value == DEFAULT {
            AudioRecoveryPolicy::FollowSystemDefault
        } else {
            AudioRecoveryPolicy::WaitForSelected
        },
        device: Some(selection),
        ..DirectionConfiguration::disabled()
    };
    configuration.physical_outputs = channels.iter().map(|channel| channel.key.clone()).collect();

    let mut device_options = vec![option(DEFAULT, DEFAULT.into())];
    device_options.extend(
        inventory
            .devices
            .iter()
            .filter(|device| device.supports(AudioDirection::Output))
            .map(|device| option(&device_token(&device.target), device.label.clone())),
    );
    let mut choices = settings
        .device
        .update(&device_options.into())
        .into_iter()
        .collect::<Vec<_>>();

    let mut ports = vec![option(UNASSIGNED, UNASSIGNED.into())];
    ports.extend(
        channels
            .iter()
            .map(|channel| option(channel.key.as_str(), channel.label.clone())),
    );
    let ports = ports.into();
    let mut seen = HashSet::new();
    for (index, output) in settings.outputs.iter().enumerate() {
        let id = output
            .key
            .parse::<AudioChannelId>()
            .map_err(|error| error.to_string())?;
        if !seen.insert(id) {
            return Err("Duplicate virtual audio output identity".into());
        }
        configuration.virtual_outputs.push(VirtualOutputChannel {
            id,
            label: output.label.clone(),
            gain: GainDb::UNITY,
        });
        let lane = u16::try_from(index).map_err(|_| "Too many playback channels")?;
        // The playback patch addresses planar lanes in configuration order. Its local route IDs
        // use a separate UUID version from authored virtual channel IDs (v4).
        let lane_id = uuid::Uuid::from_u128(0xdcf42302_11ec_8d43_88ce_09c514d00000 + u128::from(lane));
        configuration.playback_patch.push(PlaybackRoute {
            id: AudioRouteId::from_uuid(lane_id),
            source_channel: lane,
            destination: id,
            gain: GainDb::UNITY,
        });
        if let Some(channel) = channels
            .iter()
            .find(|channel| channel.key.as_str() == output.physical.value)
        {
            configuration.output_patch.push(OutputPatchRoute {
                id: AudioRouteId::from_uuid(id.as_uuid()),
                source: id,
                destination: channel.key.clone(),
                gain: GainDb::UNITY,
            });
        }
        choices.extend(output.physical.update(&ports));
    }
    Ok((configuration, choices))
}
