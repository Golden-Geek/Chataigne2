use std::mem::size_of;

use super::AudioObservationSnapshot;
use crate::{AnalysisResult, AudioBackendStatus, AudioDeviceDescriptor, AudioDeviceInspectorState, AudioStreamStatus};

use crate::storage::Budget;

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

impl AudioDeviceInspectorState {
    /// Bounded device metadata inspection independent of metering storage.
    #[must_use]
    pub fn retained_bytes_within(&self, maximum_bytes: usize, maximum_items: usize) -> Option<usize> {
        let mut budget = Budget {
            bytes: 0,
            maximum: maximum_bytes,
            items: maximum_items,
        };
        budget.items(1)?;
        budget.add(size_of::<Self>())?;
        inspector(&mut budget, self)?;
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
    budget.fingerprint(fingerprint)?;
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
