use super::*;

fn folders(values: &[(&str, &str)]) -> Vec<FolderSpec> {
    values
        .iter()
        .map(|(label, decl)| FolderSpec {
            label: (*label).into(),
            decl: (*decl).into(),
        })
        .collect()
}
fn leaf(path: Vec<FolderSpec>, label: String, decl: String, value: ParamValue, range: Option<(i32, i32)>) -> Write {
    Write {
        target: Target::Path(path),
        value: Value::Small(value),
        label,
        decl,
        range,
        read_only: false,
        rotary: None,
        initialized: false,
    }
}
fn channel_path(channel: u8) -> Vec<FolderSpec> {
    vec![FolderSpec {
        label: channel_folder_label(channel),
        decl: channel_decl_id(channel),
    }]
}
impl MidiModule {
    pub(super) fn input_writes(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        input: &PreparedInput,
        received_at: Duration,
    ) -> Vec<Write> {
        let mut writes = Vec::with_capacity(12);
        match &input.message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            }
            | MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => {
                let played = matches!(input.message, MidiMessage::NoteOn { velocity: 1..=127, .. });
                if played {
                    self.active_notes.insert((*channel, *note));
                    self.last_note_on = Some(LastNoteInfo {
                        channel: *channel,
                        note: *note,
                        velocity: *velocity,
                    });
                } else {
                    self.active_notes.remove(&(*channel, *note));
                }
                if played && self.note_played.is_bound() {
                    writes.push(Write::direct(self.note_played.id(), ParamValue::Trigger()));
                }
                for (target, value) in [
                    (
                        self.current_note_on.is_bound().then(|| self.current_note_on.id()),
                        clamp_usize_to_i32(self.active_notes.len()),
                    ),
                    (
                        self.last_channel.is_bound().then(|| self.last_channel.id()),
                        self.last_note_on.map_or(0, |n| i32::from(n.channel)),
                    ),
                    (
                        self.last_pitch.is_bound().then(|| self.last_pitch.id()),
                        self.last_note_on.map_or(0, |n| i32::from(n.note)),
                    ),
                    (
                        self.last_velocity.is_bound().then(|| self.last_velocity.id()),
                        self.last_note_on.map_or(0, |n| i32::from(n.velocity)),
                    ),
                ] {
                    if let Some(id) = target {
                        writes.push(Write::direct(id, ParamValue::Int(value)));
                    }
                }
                let mut path = channel_path(*channel);
                path.extend(folders(&[("Notes", NOTES_FOLDER_DECL_ID)]));
                writes.push(leaf(
                    path,
                    note_label(*note),
                    note_decl_id(*note),
                    ParamValue::Int(if played { i32::from(*velocity) } else { 0 }),
                    Some(MIDI_7_BIT_VALUE_RANGE),
                ));
            }
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => {
                let is_lsb = self.cc_channel_is_14_bit(snapshot, *channel) && (32..=63).contains(controller);
                let controller = if is_lsb { controller - 32 } else { *controller };
                let current_id = self
                    .find_cc_folder(snapshot, *channel)
                    .and_then(|folder| snapshot.find_child_by_decl_id(folder, &cc_decl_id(controller)));
                let config = current_id.map(|id| midi_cc_config(snapshot, id));
                let current = current_id
                    .and_then(|id| snapshot.node(id))
                    .and_then(|n| n.param_value.as_ref())
                    .and_then(|v| match v {
                        ParamValue::Int(v) => Some(*v),
                        _ => None,
                    })
                    .unwrap_or(0);
                let next = if let Some(config) = config.filter(|c| c.rotary_mechanism != ROTARY_ABSOLUTE) {
                    current.saturating_add(decode_rotary_delta(config.rotary_mechanism, *value).unwrap_or(0))
                } else if self.cc_parameter_is_14_bit(snapshot, *channel, controller) {
                    let combined = clamp_i32_to_u14(current);
                    let state = &mut self.cc_14_bit_state[usize::from(channel.saturating_sub(1).min(15))]
                        [usize::from(controller)];
                    if is_lsb {
                        state.lsb = Some(*value);
                    } else {
                        state.msb = Some(*value);
                    }
                    i32::from(
                        (u16::from(state.msb.unwrap_or((combined >> 7) as u8)) << 7)
                            | u16::from(state.lsb.unwrap_or((combined & 127) as u8)),
                    )
                } else {
                    i32::from(*value)
                };
                let mut path = channel_path(*channel);
                path.extend(folders(&[("Control Change", CONTROL_CHANGE_FOLDER_DECL_ID)]));
                let mut write = leaf(
                    path,
                    cc_label(controller),
                    cc_decl_id(controller),
                    ParamValue::Int(next),
                    self.cc_parameter_range(snapshot, *channel, controller),
                );
                write.rotary = Some(config.map_or(ROTARY_ABSOLUTE, |c| c.rotary_mechanism));
                writes.push(write);
            }
            MidiMessage::PolyPressure {
                channel,
                note,
                pressure,
            } => {
                let mut path = channel_path(*channel);
                path.extend(folders(&[("Poly Pressure", POLY_PRESSURE_FOLDER_DECL_ID)]));
                writes.push(leaf(
                    path,
                    note_label(*note),
                    note_decl_id(*note),
                    ParamValue::Int(i32::from(*pressure)),
                    Some(MIDI_7_BIT_VALUE_RANGE),
                ));
            }
            MidiMessage::ProgramChange { channel, program } => writes.push(leaf(
                channel_path(*channel),
                "Program".into(),
                PROGRAM_DECL_ID.into(),
                ParamValue::Int(i32::from(*program)),
                Some(MIDI_7_BIT_VALUE_RANGE),
            )),
            MidiMessage::ChannelPressure { channel, pressure } => writes.push(leaf(
                channel_path(*channel),
                "Channel Pressure".into(),
                CHANNEL_PRESSURE_DECL_ID.into(),
                ParamValue::Int(i32::from(*pressure)),
                Some(MIDI_7_BIT_VALUE_RANGE),
            )),
            MidiMessage::PitchBend { channel, value } => writes.push(leaf(
                channel_path(*channel),
                "Pitch Bend".into(),
                PITCH_BEND_DECL_ID.into(),
                ParamValue::Int(i32::from(*value)),
                Some(MIDI_14_BIT_VALUE_RANGE),
            )),
            MidiMessage::System(system) => {
                let path = folders(&[("System", SYSTEM_FOLDER_DECL_ID)]);
                let simple = match system {
                    MidiSystemMessage::SongPosition { position } => Some((
                        "Song Position",
                        SONG_POSITION_DECL_ID,
                        ParamValue::Int(i32::from(*position)),
                        Some(MIDI_14_BIT_VALUE_RANGE),
                    )),
                    MidiSystemMessage::SongSelect { song } => Some((
                        "Song Select",
                        SONG_SELECT_DECL_ID,
                        ParamValue::Int(i32::from(*song)),
                        Some(MIDI_7_BIT_VALUE_RANGE),
                    )),
                    MidiSystemMessage::TuneRequest => {
                        Some(("Tune Request", TUNE_REQUEST_DECL_ID, ParamValue::Trigger(), None))
                    }
                    MidiSystemMessage::ActiveSensing => {
                        Some(("Active Sensing", ACTIVE_SENSING_DECL_ID, ParamValue::Trigger(), None))
                    }
                    _ => None,
                };
                if let Some((label, decl, value, range)) = simple {
                    writes.push(leaf(path, label.into(), decl.into(), value, range));
                } else if let MidiSystemMessage::Sysex { bytes } = system {
                    let path = folders(&[("System", SYSTEM_FOLDER_DECL_ID), ("SysEx", SYSEX_FOLDER_DECL_ID)]);
                    let mut value = leaf(
                        path.clone(),
                        "Bytes".into(),
                        SYSEX_BYTES_DECL_ID.into(),
                        ParamValue::Str(String::new()),
                        None,
                    );
                    value.value = Value::Hex;
                    value.read_only = true;
                    writes.push(value);
                    writes.push(leaf(
                        path,
                        "Length".into(),
                        SYSEX_LENGTH_DECL_ID.into(),
                        ParamValue::Int(i32::try_from(bytes.len()).unwrap_or(i32::MAX)),
                        Some(MIDI_NONNEGATIVE_INT_RANGE),
                    ));
                } else {
                    // Clock/MTC domain state advances once. Convert only its small
                    // direct writes into the same owning cursor as other messages.
                    let start = ctx.edits.pending.len();
                    let result = match system {
                        MidiSystemMessage::TimeCodeQuarterFrame { value } => {
                            self.apply_time_code_quarter_frame(ctx, snapshot, *value, received_at)
                        }
                        MidiSystemMessage::TimingClock => {
                            self.apply_midi_clock_tick(ctx, snapshot, received_at, input.clock_timing)
                        }
                        MidiSystemMessage::Start => {
                            self.apply_midi_clock_transport_event(ctx, snapshot, START_DECL_ID, true, true)
                        }
                        MidiSystemMessage::Continue => {
                            self.apply_midi_clock_transport_event(ctx, snapshot, CONTINUE_DECL_ID, true, false)
                        }
                        MidiSystemMessage::Stop => {
                            self.apply_midi_clock_transport_event(ctx, snapshot, STOP_DECL_ID, false, false)
                        }
                        MidiSystemMessage::Reset => {
                            self.apply_midi_clock_transport_event(ctx, snapshot, RESET_DECL_ID, false, true)
                        }
                        _ => unreachable!(),
                    };
                    for queued in ctx.edits.pending.split_off(start) {
                        let Edit::SetParam { node, value, .. } = queued.edit else {
                            unreachable!("MIDI clock direct writes")
                        };
                        writes.push(Write::direct(node, value));
                    }
                    if result == MidiApplyResult::Retry {
                        logerror!(
                            "MIDI fixed clock/MTC fields are absent; available writes and callbacks are retained"
                        );
                    }
                }
            }
        }
        assert!(writes.len() <= 12, "bounded MIDI message shape");
        writes
    }
}
