use super::*;

impl MidiModule {
    pub(super) fn feedback_messages(
        &self,
        snapshot: &ProcessTreeSnapshot,
        values: NodeId,
        param: NodeId,
        value: &ParamValue,
        old: &ParamValue,
    ) -> Option<Vec<MidiMessage>> {
        let node = snapshot.node(param)?;
        let parent = snapshot.node(node.parent?)?;
        if parent.node_type != "folder" || parent.decl_id.len() > 128 || node.decl_id.len() > 128 {
            return None;
        }
        let message = if matches!(
            parent.decl_id.as_str(),
            NOTES_FOLDER_DECL_ID | CONTROL_CHANGE_FOLDER_DECL_ID | POLY_PRESSURE_FOLDER_DECL_ID
        ) {
            // MIDI channel paths have a fixed shape. Do not walk arbitrary authored
            // ancestors or infer a channel from an unrelated folder's label.
            let channel_node = snapshot.node(parent.parent?)?;
            let channel = channel(snapshot, channel_node.id, values)?;
            let ParamValue::Int(value) = value else { return None };
            if node.decl_id.len() > 8 {
                return None;
            }
            match parent.decl_id.as_str() {
                NOTES_FOLDER_DECL_ID => {
                    let note = note_from_decl_id(&node.decl_id)?;
                    let velocity = clamp_i32_to_u7(*value);
                    if velocity == 0 {
                        MidiMessage::NoteOff {
                            channel,
                            note,
                            velocity: 0,
                        }
                    } else {
                        MidiMessage::NoteOn {
                            channel,
                            note,
                            velocity,
                        }
                    }
                }
                POLY_PRESSURE_FOLDER_DECL_ID => MidiMessage::PolyPressure {
                    channel,
                    note: note_from_decl_id(&node.decl_id)?,
                    pressure: clamp_i32_to_u7(*value),
                },
                CONTROL_CHANGE_FOLDER_DECL_ID => {
                    let controller = cc_from_decl_id(&node.decl_id)?;
                    let mechanism = midi_cc_config(snapshot, param).rotary_mechanism;
                    if mechanism != ROTARY_ABSOLUTE {
                        let ParamValue::Int(old) = old else { return None };
                        MidiMessage::ControlChange {
                            channel,
                            controller,
                            value: encode_rotary_delta(mechanism, value.saturating_sub(*old))?,
                        }
                    } else if self.cc_channel_is_14_bit_handle(channel) && cc_supports_14_bit(controller) {
                        return Some(encode_14_bit_control_change(
                            channel,
                            controller,
                            clamp_i32_to_u14(*value),
                        ));
                    } else if self.cc_channel_is_14_bit_handle(channel) && (32..=63).contains(&controller) {
                        return None;
                    } else {
                        MidiMessage::ControlChange {
                            channel,
                            controller,
                            value: clamp_i32_to_u7(*value),
                        }
                    }
                }
                _ => unreachable!(),
            }
        } else if parent.parent == Some(values)
            && matches!(parent.decl_id.rsplit('/').next()?, SYSTEM_FOLDER_DECL_ID | "midi_clock")
        {
            MidiMessage::System(system_message(node.decl_id.rsplit('/').next()?, value)?)
        } else {
            let channel = channel(snapshot, parent.id, values)?;
            let ParamValue::Int(value) = value else { return None };
            match node.decl_id.as_str() {
                PROGRAM_DECL_ID => MidiMessage::ProgramChange {
                    channel,
                    program: clamp_i32_to_u7(*value),
                },
                CHANNEL_PRESSURE_DECL_ID => MidiMessage::ChannelPressure {
                    channel,
                    pressure: clamp_i32_to_u7(*value),
                },
                PITCH_BEND_DECL_ID => MidiMessage::PitchBend {
                    channel,
                    value: clamp_i32_to_u14(*value),
                },
                _ => return None,
            }
        };
        Some(vec![message])
    }
}

fn channel(snapshot: &ProcessTreeSnapshot, id: NodeId, values: NodeId) -> Option<u8> {
    let node = snapshot.node(id)?;
    if node.node_type != "folder" || node.parent != Some(values) || node.decl_id.len() > 10 {
        return None;
    }
    node.decl_id
        .strip_prefix("channel_")?
        .parse::<u8>()
        .ok()
        .filter(|v| (1..=16).contains(v))
}

fn system_message(decl: &str, value: &ParamValue) -> Option<MidiSystemMessage> {
    Some(match (decl, value) {
        (TIME_CODE_QUARTER_FRAME_DECL_ID, ParamValue::Int(value)) => MidiSystemMessage::TimeCodeQuarterFrame {
            value: clamp_i32_to_u7(*value),
        },
        (SONG_POSITION_DECL_ID, ParamValue::Int(value)) => MidiSystemMessage::SongPosition {
            position: clamp_i32_to_u14(*value),
        },
        (SONG_SELECT_DECL_ID, ParamValue::Int(value)) => MidiSystemMessage::SongSelect {
            song: clamp_i32_to_u7(*value),
        },
        (TUNE_REQUEST_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::TuneRequest,
        (TIMING_CLOCK_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::TimingClock,
        (START_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::Start,
        (CONTINUE_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::Continue,
        (STOP_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::Stop,
        (ACTIVE_SENSING_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::ActiveSensing,
        (RESET_DECL_ID, ParamValue::Trigger()) => MidiSystemMessage::Reset,
        _ => return None,
    })
}
