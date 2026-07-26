use super::*;

pub(super) fn collect_bindings(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
) -> SoundCardValueBindings {
    let mut bindings = SoundCardValueBindings {
        input_device: find_path(snapshot, module, "connection/input_device"),
        output_device: find_path(snapshot, module, "connection/output_device"),
        input_readiness: find_path(snapshot, module, "connection/input_readiness"),
        output_readiness: find_path(snapshot, module, "connection/output_readiness"),
        negotiated_input_format: find_path(snapshot, module, "connection/negotiated_input_format"),
        negotiated_output_format: find_path(snapshot, module, "connection/negotiated_output_format"),
        input_global_max_rms: find_path(snapshot, module, "values/global_levels/input_global_max_rms"),
        output_global_max_rms: find_path(snapshot, module, "values/global_levels/output_global_max_rms"),
        global_max_rms: find_path(snapshot, module, "values/global_levels/global_max_rms"),
        active_voices: find_path(snapshot, module, "values/playback_status/active_voices"),
        loading_voices: find_path(snapshot, module, "values/playback_status/loading_voices"),
        xruns: find_path(snapshot, module, "values/diagnostics/xruns"),
        dropped_analysis_frames: find_path(snapshot, module, "values/diagnostics/dropped_analysis_frames"),
        last_error: find_path(snapshot, module, "values/diagnostics/last_error"),
        ..SoundCardValueBindings::default()
    };

    for parent in [
        find_path(snapshot, module, INPUT_LEVELS_PATH),
        find_path(snapshot, module, OUTPUT_LEVELS_PATH),
    ]
    .into_iter()
    .flatten()
    {
        for meter in typed_children(snapshot, Some(parent), SoundCardChannelMeter::NODE_TYPE) {
            let Some(reference) = child_reference(snapshot, meter, "channel") else {
                continue;
            };
            bindings.meters.insert(
                AudioChannelId::from_uuid(reference.uuid().0),
                MeterBinding {
                    linear_rms: child_id(snapshot, meter, "linear_rms"),
                    rms_dbfs: child_id(snapshot, meter, "rms_dbfs"),
                    peak_dbfs: child_id(snapshot, meter, "peak_dbfs"),
                    clipped: child_id(snapshot, meter, "clipped"),
                },
            );
        }
    }

    if let Some(analysis) = find_path(snapshot, module, ANALYSIS_PATH) {
        for analyzer in snapshot.child_ids_slice(analysis) {
            let Some(state) = snapshot.node(*analyzer) else {
                continue;
            };
            let tap = AnalysisTapId::from_uuid(state.uuid.0);
            if state.node_type == SoundCardPitchAnalyzer::NODE_TYPE {
                if let Some(result) = child_by_uuid(
                    snapshot,
                    find_path(snapshot, module, PITCH_RESULTS_PATH),
                    pitch_result_uuid(state.uuid),
                ) {
                    bindings.pitch.insert(
                        tap,
                        PitchBinding {
                            valid: child_id(snapshot, result, "valid"),
                            frequency_hz: child_id(snapshot, result, "frequency_hz"),
                            confidence: child_id(snapshot, result, "confidence"),
                            midi_note: child_id(snapshot, result, "midi_note"),
                            note_name: child_id(snapshot, result, "note_name"),
                            cents: child_id(snapshot, result, "cents"),
                        },
                    );
                }
            } else if state.node_type == SoundCardSpectrumAnalyzer::NODE_TYPE {
                if let Some(result) = child_by_uuid(
                    snapshot,
                    find_path(snapshot, module, SPECTRUM_RESULTS_PATH),
                    spectrum_result_uuid(state.uuid),
                ) {
                    let bands =
                        typed_children(snapshot, Some(result), SoundCardSpectrumBand::NODE_TYPE)
                            .into_iter()
                            .map(|band| SpectrumBandBinding {
                                low_hz: child_id(snapshot, band, "low_hz"),
                                center_hz: child_id(snapshot, band, "center_hz"),
                                high_hz: child_id(snapshot, band, "high_hz"),
                                linear_amplitude: child_id(snapshot, band, "linear_amplitude"),
                                dbfs: child_id(snapshot, band, "dbfs"),
                            })
                            .collect();
                    bindings.spectrum.insert(tap, bands);
                }
            }
        }
    }

    bindings.runtime_values.extend(
        [
            find_path(snapshot, module, "connection/connected"),
            find_path(snapshot, module, "connection/can_receive"),
            find_path(snapshot, module, "connection/can_send"),
            bindings.input_readiness,
            bindings.output_readiness,
            bindings.negotiated_input_format,
            bindings.negotiated_output_format,
            bindings.input_global_max_rms,
            bindings.output_global_max_rms,
            bindings.global_max_rms,
            bindings.active_voices,
            bindings.loading_voices,
            bindings.xruns,
            bindings.dropped_analysis_frames,
            bindings.last_error,
        ]
        .into_iter()
        .flatten(),
    );
    for meter in bindings.meters.values() {
        bindings.runtime_values.extend([
            meter.linear_rms,
            meter.rms_dbfs,
            meter.peak_dbfs,
            meter.clipped,
        ]);
    }
    for pitch in bindings.pitch.values() {
        bindings.runtime_values.extend([
            pitch.valid,
            pitch.frequency_hz,
            pitch.confidence,
            pitch.midi_note,
            pitch.note_name,
            pitch.cents,
        ]);
    }
    for band in bindings.spectrum.values().flatten() {
        bindings.runtime_values.extend([
            band.low_hz,
            band.center_hz,
            band.high_hz,
            band.linear_amplitude,
            band.dbfs,
        ]);
    }
    bindings
}

pub(super) fn telemetry_from_observation(
    observation: &AudioObservationSnapshot,
    playback_voices: Vec<SoundCardPlaybackVoiceDto>,
) -> SoundCardUiTelemetryDto {
    SoundCardUiTelemetryDto {
        generation: observation.generation,
        render_frame: observation.render_frame,
        device: observation.device.clone(),
        inputs: observation.inputs.clone(),
        outputs: observation.outputs.clone(),
        input_global_max_rms: observation.input_global_max_rms,
        output_global_max_rms: observation.output_global_max_rms,
        global_max_rms: observation.global_max_rms,
        playback: observation.playback,
        runtime: observation.runtime,
        playback_source_channel_limit: MAX_PLAYBACK_SOURCE_CHANNELS,
        playback_voices,
        dropped_event_count: observation.dropped_event_count,
        queue_pressure_count: observation.queue_pressure_count,
        analysis: observation.analysis.clone(),
    }
}

pub(super) fn values_nearly_equal(current: &ParamValue, next: &ParamValue) -> bool {
    match (current, next) {
        (ParamValue::Float(left), ParamValue::Float(right)) => {
            (left - right).abs() <= OBSERVATION_EPSILON
        }
        _ => current == next,
    }
}

pub(super) fn readiness_name(readiness: AudioDeviceReadiness) -> String {
    serde_json::to_value(readiness)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

pub(super) fn format_description(
    format: Option<&golden_audio::NegotiatedStreamFormat>,
) -> String {
    format.map_or_else(String::new, |format| {
        format!(
            "{} Hz | {} ch | {:?} | {} frames | {:.2} ms",
            format.sample_rate,
            format.channels,
            format.sample_format,
            format.buffer_frames,
            format.estimated_latency_ms
        )
    })
}

pub(super) fn saturating_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
