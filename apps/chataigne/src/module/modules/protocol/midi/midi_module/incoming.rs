use super::*;

impl MidiModule {
    pub(super) fn process_pending_incoming(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        if self.pending_incoming_messages.is_empty() {
            return false;
        }

        let mut remaining = Vec::new();
        let mut messages = std::mem::take(&mut self.pending_incoming_messages).into_iter();
        while let Some(pending) = messages.next() {
            let received_at = pending.received_at.unwrap_or(ctx.runtime_elapsed);
            match self.apply_incoming_message(
                ctx,
                snapshot,
                &pending.message,
                received_at,
                pending.clock_timing.as_ref(),
            ) {
                MidiApplyResult::Applied | MidiApplyResult::Ignored => {
                    self.emit_midi_received_callbacks(ctx, &pending.message);
                }
                MidiApplyResult::Retry => {
                    remaining.push(pending);
                    remaining.extend(messages);
                    self.pending_incoming_messages = remaining;
                    return true;
                }
            }
        }

        self.pending_incoming_messages = remaining;
        false
    }

    pub(super) fn apply_incoming_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &MidiMessage,
        received_at: Duration,
        clock_timing: Option<&MidiClockTiming>,
    ) -> MidiApplyResult {
        match message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } if *velocity == 0 => self.apply_note_off(ctx, snapshot, *channel, *note, 0),
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => self.apply_note_on(ctx, snapshot, *channel, *note, *velocity),
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => self.apply_note_off(ctx, snapshot, *channel, *note, *velocity),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => self.apply_control_change(ctx, snapshot, *channel, *controller, *value),
            MidiMessage::PolyPressure {
                channel,
                note,
                pressure,
            } => self.apply_poly_pressure(ctx, snapshot, *channel, *note, *pressure),
            MidiMessage::ProgramChange { channel, program } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                MidiIntValue {
                    label: "Program",
                    decl_id: PROGRAM_DECL_ID,
                    value: i32::from(*program),
                    range: Some((0, 127)),
                },
            ),
            MidiMessage::ChannelPressure { channel, pressure } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                MidiIntValue {
                    label: "Channel Pressure",
                    decl_id: CHANNEL_PRESSURE_DECL_ID,
                    value: i32::from(*pressure),
                    range: Some((0, 127)),
                },
            ),
            MidiMessage::PitchBend { channel, value } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                MidiIntValue {
                    label: "Pitch Bend",
                    decl_id: PITCH_BEND_DECL_ID,
                    value: i32::from(*value),
                    range: Some((0, i32::from(MIDI_U14_MAX))),
                },
            ),
            MidiMessage::System(system) => self.apply_system_message(ctx, snapshot, system, received_at, clock_timing),
        }
    }

    pub(super) fn apply_note_on(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        velocity: u8,
    ) -> MidiApplyResult {
        self.active_notes.insert((channel, note));
        self.last_note_on = Some(LastNoteInfo {
            channel,
            note,
            velocity,
        });

        let note_info_result = self.apply_note_info(ctx, snapshot, true);
        if note_info_result == MidiApplyResult::Retry {
            return note_info_result;
        }

        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(notes_id) = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        merge_apply_results(
            note_info_result,
            self.apply_direct_int_parameter(
                ctx,
                snapshot,
                notes_id,
                MidiIntValue {
                    label: note_label(note).as_str(),
                    decl_id: note_decl_id(note).as_str(),
                    value: i32::from(velocity),
                    range: Some(MIDI_7_BIT_VALUE_RANGE),
                },
            ),
        )
    }

    pub(super) fn apply_note_off(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        _velocity: u8,
    ) -> MidiApplyResult {
        self.active_notes.remove(&(channel, note));

        let note_info_result = self.apply_note_info(ctx, snapshot, false);
        if note_info_result == MidiApplyResult::Retry {
            return note_info_result;
        }

        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(notes_id) = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        merge_apply_results(
            note_info_result,
            self.apply_direct_int_parameter(
                ctx,
                snapshot,
                notes_id,
                MidiIntValue {
                    label: note_label(note).as_str(),
                    decl_id: note_decl_id(note).as_str(),
                    value: 0,
                    range: Some(MIDI_7_BIT_VALUE_RANGE),
                },
            ),
        )
    }

    pub(super) fn apply_note_info(
        &mut self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
        note_played: bool,
    ) -> MidiApplyResult {
        if !self.current_note_on.is_bound()
            || !self.last_channel.is_bound()
            || !self.last_pitch.is_bound()
            || !self.last_velocity.is_bound()
            || (note_played && !self.note_played.is_bound())
        {
            return MidiApplyResult::Retry;
        }

        if note_played {
            self.set_internal_param(ctx, self.note_played.id(), ParamValue::Trigger());
        }

        self.set_internal_param(
            ctx,
            self.current_note_on.id(),
            ParamValue::Int(clamp_usize_to_i32(self.active_notes.len())),
        );

        let last_channel = self.last_note_on.map(|info| i32::from(info.channel)).unwrap_or(0);
        self.set_internal_param(ctx, self.last_channel.id(), ParamValue::Int(last_channel));

        let last_pitch = self.last_note_on.map(|info| i32::from(info.note)).unwrap_or(0);
        self.set_internal_param(ctx, self.last_pitch.id(), ParamValue::Int(last_pitch));

        let last_velocity = self.last_note_on.map(|info| i32::from(info.velocity)).unwrap_or(0);
        self.set_internal_param(ctx, self.last_velocity.id(), ParamValue::Int(last_velocity));
        MidiApplyResult::Applied
    }

    pub(super) fn cc_parameter_range(
        &self,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
    ) -> Option<(i32, i32)> {
        Some(if self.cc_parameter_is_14_bit(snapshot, channel, controller) {
            MIDI_14_BIT_VALUE_RANGE
        } else {
            MIDI_7_BIT_VALUE_RANGE
        })
    }

    pub(super) fn apply_control_change(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
        raw_value: u8,
    ) -> MidiApplyResult {
        if self.cc_channel_is_14_bit(snapshot, channel) && (32..=63).contains(&controller) {
            let base_controller = controller - 32;
            if self.cc_parameter_is_14_bit(snapshot, channel, base_controller) {
                return self.apply_control_change_value(ctx, snapshot, channel, base_controller, raw_value, true);
            }
        }

        self.apply_control_change_value(ctx, snapshot, channel, controller, raw_value, false)
    }

    pub(super) fn apply_control_change_value(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
        raw_value: u8,
        is_lsb_message: bool,
    ) -> MidiApplyResult {
        let Some(cc_id) = self.ensure_cc_parameter(ctx, snapshot, channel, controller) else {
            return MidiApplyResult::Retry;
        };
        let config = midi_cc_config(snapshot, cc_id);

        if config.rotary_mechanism != ROTARY_ABSOLUTE {
            let current = snapshot
                .node(cc_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
                .unwrap_or(0);
            let delta = decode_rotary_delta(config.rotary_mechanism, raw_value).unwrap_or(0);
            self.set_internal_param(ctx, cc_id, ParamValue::Int(current.saturating_add(delta)));
            return MidiApplyResult::Applied;
        }

        if self.cc_parameter_is_14_bit(snapshot, channel, controller) {
            let current_combined = snapshot
                .node(cc_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
                .map(clamp_i32_to_u14)
                .unwrap_or(0);
            let default_msb = ((current_combined >> 7) & 0x7F) as u8;
            let default_lsb = (current_combined & 0x7F) as u8;
            let channel_index = usize::from(channel.saturating_sub(1).min(15));
            let controller_index = usize::from(controller);
            let state = &mut self.cc_14_bit_state[channel_index][controller_index];
            if is_lsb_message {
                state.lsb = Some(raw_value);
            } else {
                state.msb = Some(raw_value);
            }

            let msb = state.msb.unwrap_or(default_msb);
            let lsb = state.lsb.unwrap_or(default_lsb);
            let combined = (u16::from(msb) << 7) | u16::from(lsb);
            self.set_internal_param(ctx, cc_id, ParamValue::Int(i32::from(combined)));
            return MidiApplyResult::Applied;
        }

        self.set_internal_param(ctx, cc_id, ParamValue::Int(i32::from(raw_value)));
        MidiApplyResult::Applied
    }

    pub(super) fn apply_poly_pressure(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        pressure: u8,
    ) -> MidiApplyResult {
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(pressure_folder_id) =
            self.ensure_folder(ctx, snapshot, channel_id, "Poly Pressure", POLY_PRESSURE_FOLDER_DECL_ID)
        else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            pressure_folder_id,
            MidiIntValue {
                label: note_label(note).as_str(),
                decl_id: note_decl_id(note).as_str(),
                value: i32::from(pressure),
                range: Some(MIDI_7_BIT_VALUE_RANGE),
            },
        )
    }

    pub(super) fn apply_channel_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        value: MidiIntValue<'_>,
    ) -> MidiApplyResult {
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(ctx, snapshot, channel_id, value)
    }

    pub(super) fn apply_system_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &MidiSystemMessage,
        received_at: Duration,
        clock_timing: Option<&MidiClockTiming>,
    ) -> MidiApplyResult {
        match message {
            MidiSystemMessage::TimeCodeQuarterFrame { value } => {
                self.apply_time_code_quarter_frame(ctx, snapshot, *value, received_at)
            }
            MidiSystemMessage::SongPosition { position } => self.apply_system_int_parameter(
                ctx,
                snapshot,
                MidiIntValue {
                    label: "Song Position",
                    decl_id: SONG_POSITION_DECL_ID,
                    value: i32::from(*position),
                    range: Some(MIDI_14_BIT_VALUE_RANGE),
                },
            ),
            MidiSystemMessage::SongSelect { song } => self.apply_system_int_parameter(
                ctx,
                snapshot,
                MidiIntValue {
                    label: "Song Select",
                    decl_id: SONG_SELECT_DECL_ID,
                    value: i32::from(*song),
                    range: Some(MIDI_7_BIT_VALUE_RANGE),
                },
            ),
            MidiSystemMessage::TuneRequest => {
                self.apply_system_trigger(ctx, snapshot, "Tune Request", TUNE_REQUEST_DECL_ID)
            }
            MidiSystemMessage::TimingClock => {
                self.apply_midi_clock_tick(ctx, snapshot, received_at, clock_timing.copied())
            }
            MidiSystemMessage::Start => self.apply_midi_clock_transport_event(ctx, snapshot, START_DECL_ID, true, true),
            MidiSystemMessage::Continue => {
                self.apply_midi_clock_transport_event(ctx, snapshot, CONTINUE_DECL_ID, true, false)
            }
            MidiSystemMessage::Stop => self.apply_midi_clock_transport_event(ctx, snapshot, STOP_DECL_ID, false, false),
            MidiSystemMessage::ActiveSensing => {
                self.apply_system_trigger(ctx, snapshot, "Active Sensing", ACTIVE_SENSING_DECL_ID)
            }
            MidiSystemMessage::Reset => {
                self.apply_midi_clock_transport_event(ctx, snapshot, RESET_DECL_ID, false, true)
            }
            MidiSystemMessage::Sysex { bytes } => self.apply_sysex_to_system(ctx, snapshot, bytes.as_slice()),
        }
    }

    pub(super) fn apply_system_int_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        value: MidiIntValue<'_>,
    ) -> MidiApplyResult {
        let Some(system_id) = self.ensure_system_folder(ctx, snapshot) else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(ctx, snapshot, system_id, value)
    }

    pub(super) fn apply_system_trigger(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        label: &str,
        decl_id: &str,
    ) -> MidiApplyResult {
        let Some(system_id) = self.ensure_system_folder(ctx, snapshot) else {
            return MidiApplyResult::Retry;
        };

        self.apply_trigger(ctx, snapshot, system_id, label, decl_id)
    }

    pub(super) fn ensure_system_folder(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        self.ensure_folder(ctx, snapshot, values_id, "System", SYSTEM_FOLDER_DECL_ID)
    }
}
