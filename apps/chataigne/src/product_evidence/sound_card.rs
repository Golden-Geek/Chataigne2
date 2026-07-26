use std::time::Duration;

use golden_audio::{
    qualification::{ReferenceWorkload, ReferenceWorkloadHarness},
    AudioBackend, AudioBufferPolicy, AudioDirection, AudioStreamStatus, NullBackend, SampleRate, StreamRequest,
};
use serde_json::{json, Value};

const RENDER_BLOCKS: usize = 96;
const MEMORY_BOUND_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn run() -> Result<Value, String> {
    let null_backend = NullBackend;
    let null_device = null_backend
        .discover()
        .map_err(|error| format!("failed to discover the null audio device: {error}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "null audio backend did not expose its deterministic device".to_string())?;
    let mut null_stream = null_backend
        .open_stream(&StreamRequest {
            direction: AudioDirection::Output,
            target: null_device.target.clone(),
            engine_sample_rate: SampleRate::default(),
            channels: 2,
            buffer_policy: AudioBufferPolicy::Fixed(128),
        })
        .map_err(|error| format!("failed to open the null audio stream: {error}"))?;
    null_stream
        .start()
        .map_err(|error| format!("failed to start the null audio stream: {error}"))?;
    let null_status = null_stream.status();

    let mut harness = ReferenceWorkloadHarness::new(ReferenceWorkload::Medium)
        .map_err(|error| format!("failed to build the Sound Card reference workload: {error}"))?;
    harness
        .render_blocks(RENDER_BLOCKS)
        .map_err(|error| format!("failed to render the Sound Card reference workload: {error}"))?;
    if !harness.wait_for_analysis(Duration::from_secs(2)) {
        return Err("Sound Card reference analysis did not complete before the evidence deadline".to_string());
    }
    let observation = harness.observation();
    null_stream
        .stop()
        .map_err(|error| format!("failed to stop the null audio stream: {error}"))?;

    if !observation.finite_output || observation.peak_output <= 0.0 {
        return Err("Sound Card reference workload did not produce finite non-silent output".to_string());
    }
    if observation.observed_pitch_taps != observation.specification.pitch_taps
        || observation.observed_spectrum_taps != observation.specification.spectrum_taps
    {
        return Err("Sound Card reference analysis did not publish every configured result".to_string());
    }
    if observation.estimated_resident_bytes > MEMORY_BOUND_BYTES {
        return Err(format!(
            "Sound Card medium workload estimated {} resident bytes, exceeding the {} byte evidence bound",
            observation.estimated_resident_bytes, MEMORY_BOUND_BYTES
        ));
    }

    Ok(json!({
        "scenario_version": 1,
        "device": {
            "backend": "null",
            "stable_id": null_device.stable_id,
            "ready": is_ready(&null_status),
            "sample_rate": null_status.format.as_ref().map(|format| format.sample_rate),
            "buffer_frames": null_status.format.as_ref().map(|format| format.buffer_frames),
        },
        "workload": {
            "name": observation.workload,
            "channels": observation.specification.channels,
            "routes": observation.specification.routes,
            "voices": observation.specification.voices,
            "rendered_blocks": RENDER_BLOCKS,
            "rendered_frames": observation.rendered_frames,
        },
        "signal": {
            "finite": observation.finite_output,
            "non_silent": observation.peak_output > 0.0,
            "input_metering": observation.input_global_max_rms > 0.0,
            "output_metering": observation.output_global_max_rms > 0.0,
        },
        "analysis": {
            "pitch_results": observation.observed_pitch_taps,
            "spectrum_results": observation.observed_spectrum_taps,
            "spectrum_bands": observation.specification.spectrum_bands,
        },
        "memory": {
            "bounded": observation.estimated_resident_bytes <= MEMORY_BOUND_BYTES,
            "bound_bytes": MEMORY_BOUND_BYTES,
        },
    }))
}

fn is_ready(status: &AudioStreamStatus) -> bool {
    status.enabled && status.readiness == golden_audio::AudioDeviceReadiness::Ready
}
