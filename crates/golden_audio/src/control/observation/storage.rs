use std::mem::size_of;

use super::AudioObservationSnapshot;
use crate::{
    AnalysisResult, AudioBackendStatus, AudioDeviceDescriptor, AudioDeviceFingerprint, AudioDeviceInspectorState,
    AudioDeviceTargetId, AudioStreamStatus,
};

struct Budget {
    bytes: usize,
    maximum: usize,
    items: usize,
}

impl Budget {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.bytes = self.bytes.checked_add(bytes)?;
        (self.bytes <= self.maximum).then_some(())
    }

    fn items(&mut self, count: usize) -> Option<()> {
        self.items = self.items.checked_sub(count)?;
        Some(())
    }

    fn vector<T>(&mut self, values: &Vec<T>) -> Option<()> {
        self.items(values.len())?;
        self.add(values.capacity().checked_mul(size_of::<T>())?)
    }

    fn optional(&mut self, value: &Option<String>) -> Option<()> {
        self.add(value.as_ref().map_or(0, String::capacity))
    }

    fn target(&mut self, target: &AudioDeviceTargetId) -> Option<()> {
        match target {
            AudioDeviceTargetId::SystemDefault { backend } => self.add(backend.retained_heap_bytes()),
            AudioDeviceTargetId::Device { backend, device } => {
                self.add(backend.retained_heap_bytes())?;
                self.add(device.retained_heap_bytes())
            }
        }
    }
}

impl AudioObservationSnapshot {
    /// Conservative owned storage, including spare capacities. Returns `None`
    /// before traversing more than `maximum_items` collection entries or retaining
    /// more than `maximum_bytes`. Call on a non-realtime preparing worker.
    #[must_use]
    pub fn retained_bytes_within(&self, maximum_bytes: usize, maximum_items: usize) -> Option<usize> {
        let Self {
            generation: _,
            render_frame: _,
            enabled: _,
            inputs,
            outputs,
            input_global_max_rms: _,
            output_global_max_rms: _,
            global_max_rms: _,
            playback: _,
            runtime: _,
            dropped_event_count: _,
            queue_pressure_count: _,
            device,
            analysis,
        } = self;
        let mut budget = Budget {
            bytes: 0,
            maximum: maximum_bytes,
            items: maximum_items,
        };
        budget.items(1)?;
        budget.add(size_of::<Self>())?;
        budget.vector(inputs)?;
        budget.vector(outputs)?;
        inspector(&mut budget, device)?;
        let crate::AnalysisObservationSnapshot {
            generation: _,
            render_frame: _,
            inputs,
            outputs,
            input_global_max_rms: _,
            output_global_max_rms: _,
            global_max_rms: _,
            taps,
            diagnostics: _,
        } = analysis;
        budget.vector(inputs)?;
        budget.vector(outputs)?;
        budget.vector(taps)?;
        for tap in taps {
            let crate::AnalysisTapObservation {
                tap: _,
                source: _,
                enabled: _,
                result,
            } = tap;
            match result {
                Some(AnalysisResult::Pitch(pitch)) => budget.add(pitch.note_name.capacity())?,
                Some(AnalysisResult::Spectrum(spectrum)) => budget.vector(&spectrum.bands)?,
                None => {}
            }
        }
        Some(budget.bytes)
    }
}

fn inspector(budget: &mut Budget, state: &AudioDeviceInspectorState) -> Option<()> {
    let AudioDeviceInspectorState {
        discovery_in_progress: _,
        inventory_revision: _,
        backends,
        device_catalog,
        devices,
        input,
        output,
        engine_sample_rate: _,
        buffer_policy: _,
    } = state;
    budget.vector(backends)?;
    for status in backends {
        let AudioBackendStatus {
            backend,
            label,
            state: _,
            detail,
        } = status;
        budget.add(backend.retained_heap_bytes())?;
        budget.add(label.capacity())?;
        budget.optional(detail)?;
    }
    budget.vector(device_catalog)?;
    for entry in device_catalog {
        let crate::AudioDeviceCatalogEntry { target, label } = entry;
        budget.target(target)?;
        budget.add(label.capacity())?;
    }
    budget.vector(devices)?;
    for descriptor in devices {
        device(budget, descriptor)?;
    }
    status(budget, input)?;
    status(budget, output)
}

fn device(budget: &mut Budget, device: &AudioDeviceDescriptor) -> Option<()> {
    let AudioDeviceDescriptor {
        target,
        label,
        stable_id: _,
        fingerprint,
        profile_key,
        input_channels,
        output_channels,
        supported_configurations,
        is_system_default_input: _,
        is_system_default_output: _,
    } = device;
    budget.target(target)?;
    budget.add(label.capacity())?;
    budget.add(profile_key.retained_heap_bytes())?;
    let AudioDeviceFingerprint {
        vendor,
        product,
        serial,
        transport,
        backend_path,
        input_channels: _,
        output_channels: _,
        properties,
    } = fingerprint;
    for value in [vendor, product, serial, transport, backend_path] {
        budget.optional(value)?;
    }
    budget.items(properties.len())?;
    budget.add(properties.len().checked_add(1)?.checked_mul(1024)?)?;
    for (key, value) in properties {
        budget.add(key.capacity())?;
        budget.add(value.capacity())?;
    }
    for channels in [input_channels, output_channels] {
        budget.vector(channels)?;
        for channel in channels {
            let crate::PhysicalChannelDescriptor { key, label, position } = channel;
            budget.add(key.retained_heap_bytes())?;
            budget.add(label.capacity())?;
            budget.optional(position)?;
        }
    }
    budget.vector(supported_configurations)
}

fn status(budget: &mut Budget, status: &AudioStreamStatus) -> Option<()> {
    let AudioStreamStatus {
        direction: _,
        enabled: _,
        selected_target,
        selected_label,
        profile_key,
        active_target,
        readiness: _,
        permission: _,
        recovery_policy: _,
        retry_attempt: _,
        next_retry_ms: _,
        format: _,
        error,
    } = status;
    for target in [selected_target, active_target].into_iter().flatten() {
        budget.target(target)?;
    }
    budget.optional(selected_label)?;
    if let Some(profile) = profile_key {
        budget.add(profile.retained_heap_bytes())?;
    }
    if let Some(error) = error {
        let crate::AudioInspectorError {
            category: _,
            message,
            technical_detail,
        } = error;
        budget.add(message.capacity())?;
        budget.optional(technical_detail)?;
    }
    Some(())
}
