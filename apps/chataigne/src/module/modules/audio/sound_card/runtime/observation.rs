use super::*;

pub(super) fn collect_bindings(snapshot: &ProcessTreeSnapshot, module: NodeId) -> SoundCardValueBindings {
    let mut bindings = SoundCardValueBindings {
        input_master_level: find_path(snapshot, module, "values/input/master_level"),
        output_master_level: find_path(snapshot, module, "values/output/master_level"),
        ..SoundCardValueBindings::default()
    };

    for parent in [
        find_path(snapshot, module, "parameters/input/channels"),
        find_path(snapshot, module, "parameters/output/channels"),
    ]
    .into_iter()
    .flatten()
    {
        for channel in snapshot.child_ids(parent) {
            let Some(state) = snapshot.node(channel) else {
                continue;
            };
            if !matches!(state.param_value, Some(ParamValue::Float(_))) {
                continue;
            }
            let value_uuid = NodeUuid(Uuid::new_v5(
                &state.uuid.0,
                b"sound-card-channel-value",
            ));
            if let Some(value) = snapshot.node_id_by_uuid(value_uuid) {
                bindings
                    .channel_levels
                    .insert(AudioChannelId::from_uuid(state.uuid.0), value);
            }
        }
    }

    let module_uuid = snapshot.node(module).expect("module exists").uuid;
    if let Some(result) = find_path(snapshot, module, PITCH_VALUES_PATH) {
        bindings.pitch.insert(
            pitch_tap_id(module_uuid),
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
    if let Some(result) = find_path(snapshot, module, SPECTRAL_VALUES_PATH) {
        let bands = typed_children(snapshot, Some(result), SoundCardSpectrumBand::NODE_TYPE)
            .into_iter()
            .map(|band| SpectrumBandBinding {
                low_hz: child_id(snapshot, band, "low_hz"),
                center_hz: child_id(snapshot, band, "center_hz"),
                high_hz: child_id(snapshot, band, "high_hz"),
                linear_amplitude: child_id(snapshot, band, "linear_amplitude"),
                dbfs: child_id(snapshot, band, "dbfs"),
            })
            .collect();
        bindings.spectrum.insert(spectral_tap_id(module_uuid), bands);
    }

    bindings.runtime_values.extend(
        [bindings.input_master_level, bindings.output_master_level]
            .into_iter()
            .flatten(),
    );
    bindings
        .runtime_values
        .extend(bindings.channel_levels.values().copied());
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
        (ParamValue::Float(left), ParamValue::Float(right)) => (left - right).abs() <= OBSERVATION_EPSILON,
        _ => current == next,
    }
}
