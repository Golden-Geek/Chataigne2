use serde::{Deserialize, Serialize};

use crate::{
    AudioDeviceCatalogEntry, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceProfileKey, AudioDeviceTargetId,
    AudioDirection,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioDeviceSelection {
    pub target: AudioDeviceTargetId,
    pub fallback_fingerprint: Option<AudioDeviceFingerprint>,
    pub last_known_label: String,
    pub profile_key: AudioDeviceProfileKey,
}

impl AudioDeviceSelection {
    #[must_use]
    pub fn from_descriptor(device: &AudioDeviceDescriptor) -> Self {
        Self {
            target: device.target.clone(),
            fallback_fingerprint: (!device.stable_id).then(|| device.fingerprint.clone()),
            last_known_label: device.label.clone(),
            profile_key: device.profile_key.clone(),
        }
    }

    #[must_use]
    pub fn from_catalog_entry(device: &AudioDeviceCatalogEntry) -> Self {
        Self {
            profile_key: profile_key_for(&device.target, None, None),
            target: device.target.clone(),
            fallback_fingerprint: None,
            last_known_label: device.label.clone(),
        }
    }

    #[must_use]
    pub fn follow_system_default(backend: crate::BackendId, direction: AudioDirection) -> Self {
        let target = AudioDeviceTargetId::SystemDefault { backend };
        Self {
            profile_key: profile_key_for(&target, None, Some(direction)),
            target,
            fallback_fingerprint: None,
            last_known_label: "System Default".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioDeviceMatch {
    Matched(Box<AudioDeviceDescriptor>),
    Missing,
    Ambiguous(Vec<AudioDeviceTargetId>),
}

#[must_use]
pub fn match_device_selection(
    selection: &AudioDeviceSelection,
    direction: AudioDirection,
    devices: &[AudioDeviceDescriptor],
) -> AudioDeviceMatch {
    match &selection.target {
        AudioDeviceTargetId::SystemDefault { backend } => {
            let matches = devices
                .iter()
                .filter(|device| {
                    device.target.backend() == backend
                        && device.supports(direction)
                        && match direction {
                            AudioDirection::Input => device.is_system_default_input,
                            AudioDirection::Output => device.is_system_default_output,
                        }
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => AudioDeviceMatch::Missing,
                [device] => AudioDeviceMatch::Matched(Box::new((*device).clone())),
                _ => AudioDeviceMatch::Ambiguous(matches.into_iter().map(|device| device.target.clone()).collect()),
            }
        }
        AudioDeviceTargetId::Device { backend, .. } => {
            if let Some(exact) = devices
                .iter()
                .find(|device| device.target == selection.target && device.supports(direction))
            {
                return AudioDeviceMatch::Matched(Box::new(exact.clone()));
            }
            let Some(fingerprint) = &selection.fallback_fingerprint else {
                return AudioDeviceMatch::Missing;
            };
            let matches = devices
                .iter()
                .filter(|device| {
                    !device.stable_id
                        && device.target.backend() == backend
                        && device.supports(direction)
                        && &device.fingerprint == fingerprint
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => AudioDeviceMatch::Missing,
                [device] => AudioDeviceMatch::Matched(Box::new((*device).clone())),
                _ => AudioDeviceMatch::Ambiguous(matches.into_iter().map(|device| device.target.clone()).collect()),
            }
        }
    }
}

#[must_use]
pub fn profile_key_for(
    target: &AudioDeviceTargetId,
    fingerprint: Option<&AudioDeviceFingerprint>,
    direction: Option<AudioDirection>,
) -> AudioDeviceProfileKey {
    let mut canonical = String::new();
    append_component(&mut canonical, target.backend().as_str());
    match target {
        AudioDeviceTargetId::SystemDefault { .. } => append_component(&mut canonical, "default"),
        AudioDeviceTargetId::Device { device, .. } => append_component(&mut canonical, device.as_str()),
    }
    if let Some(direction) = direction {
        append_component(
            &mut canonical,
            match direction {
                AudioDirection::Input => "input",
                AudioDirection::Output => "output",
            },
        );
    }
    if let Some(fingerprint) = fingerprint {
        append_fingerprint(&mut canonical, fingerprint);
    }
    let first = fnv1a64(canonical.as_bytes(), 0xcbf2_9ce4_8422_2325);
    let second = fnv1a64(canonical.as_bytes(), 0x8422_2325_cbf2_9ce4);
    AudioDeviceProfileKey::new(format!("device-{first:016x}{second:016x}"))
        .expect("generated device profile keys are non-empty and trimmed")
}

fn append_fingerprint(canonical: &mut String, fingerprint: &AudioDeviceFingerprint) {
    for value in [
        fingerprint.vendor.as_deref(),
        fingerprint.product.as_deref(),
        fingerprint.serial.as_deref(),
        fingerprint.transport.as_deref(),
        fingerprint.backend_path.as_deref(),
    ] {
        append_component(canonical, value.unwrap_or_default());
    }
    append_component(canonical, fingerprint.input_channels.to_string().as_str());
    append_component(canonical, fingerprint.output_channels.to_string().as_str());
    for (key, value) in &fingerprint.properties {
        append_component(canonical, key);
        append_component(canonical, value);
    }
}

fn append_component(output: &mut String, value: &str) {
    output.push_str(value.len().to_string().as_str());
    output.push(':');
    output.push_str(value);
    output.push('|');
}

fn fnv1a64(bytes: &[u8], seed: u64) -> u64 {
    bytes.iter().fold(seed, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
