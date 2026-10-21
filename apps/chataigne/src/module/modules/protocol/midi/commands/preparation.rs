use super::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::node::NodeId;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MidiSendRequest {
    pub packets: Vec<MidiSendPacket>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MidiSendPacket {
    pub bytes: Vec<u8>,
    pub description: String,
    pub delay_ms: u64,
}

impl MidiSendRequest {
    pub(crate) fn immediate(message: MidiMessage) -> Self {
        Self {
            packets: vec![MidiSendPacket::immediate(message)],
        }
    }

    pub(crate) fn immediate_many(messages: Vec<MidiMessage>, description: String) -> Self {
        Self {
            packets: messages
                .into_iter()
                .map(|message| MidiSendPacket {
                    bytes: encode_midi_message(&message),
                    description: description.clone(),
                    delay_ms: 0,
                })
                .collect(),
        }
    }
}

impl MidiSendPacket {
    pub(crate) fn immediate(message: MidiMessage) -> Self {
        Self {
            bytes: encode_midi_message(&message),
            description: message_description(&message),
            delay_ms: 0,
        }
    }

    pub(crate) fn delayed(message: MidiMessage, delay_ms: u64) -> Self {
        Self {
            bytes: encode_midi_message(&message),
            description: message_description(&message),
            delay_ms,
        }
    }

    pub(crate) fn immediate_bytes(bytes: Vec<u8>, description: impl Into<String>) -> Self {
        Self {
            bytes,
            description: description.into(),
            delay_ms: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct MidiNoteTarget {
    channel: u8,
    note: u8,
}

struct View<'a> {
    snapshot: &'a ProcessTreeSnapshot,
    execution: Option<ModuleCommandExecuteEvent>,
}
impl std::ops::Deref for View<'_> {
    type Target = ProcessTreeSnapshot;
    fn deref(&self) -> &Self::Target {
        self.snapshot
    }
}
impl View<'_> {
    fn value(&self, id: NodeId) -> Option<&ParamValue> {
        let original = self.snapshot.node(id)?.param_value.as_ref()?;
        Some(
            self.execution
                .as_ref()
                .and_then(|e| e.param_overrides.iter().rev().find(|p| p.param_id == id))
                .map(|p| &p.value)
                .unwrap_or(original),
        )
    }
}
pub(crate) fn prepare_midi_command(invocation: &CapturedInvocation) -> Result<MidiSendRequest, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|e| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**e).clone())
                .map_err(|_| "Invalid MIDI command overrides".to_string())
        })
        .transpose()?;
    if execution.as_ref().is_some_and(|e| e.command_id != invocation.command) {
        return Err("MIDI invocation targets another command".into());
    }
    let view = View {
        snapshot: &invocation.snapshot,
        execution,
    };
    let snapshot = &view;
    let command = invocation.command;
    let mut weight = 0usize;
    for path in [
        "bytes",
        "channel",
        "controller",
        "duration_ms",
        "is_14_bit",
        "message",
        "note",
        "note_mode",
        "octave",
        "off_velocity",
        "pitch",
        "pressure",
        "program",
        "quarter_frame",
        "song",
        "song_position",
        "text",
        "value",
        "value_14_bit",
        "velocity",
    ] {
        if let Some(value) = crate::app::module_command::resolve_module_command_child(snapshot, command, path)
            .and_then(|id| view.value(id))
        {
            weight = weight.saturating_add(value.retained_bytes());
            if weight > 256 * 1024 {
                return Err("MIDI command source exceeds 256 KiB".into());
            }
            if matches!(path, "note_mode" | "note" | "message")
                && matches!(value,ParamValue::Str(s)|ParamValue::Enum(s) if s.len()>128)
            {
                return Err("MIDI selector exceeds 128 bytes".into());
            }
        }
    }
    match snapshot.node(command).map(|node| node.node_type.as_str()) {
        Some(crate::app::MidiSendNoteOnCommand::NODE_TYPE) => {
            let target = command_note_target(snapshot, command)?;
            Ok(MidiSendRequest::immediate(MidiMessage::NoteOn {
                channel: target.channel,
                note: target.note,
                velocity: command_u7(snapshot, command, "velocity", DEFAULT_VELOCITY),
            }))
        }
        Some(crate::app::MidiSendNoteOffCommand::NODE_TYPE) => {
            let target = command_note_target(snapshot, command)?;
            Ok(MidiSendRequest::immediate(MidiMessage::NoteOff {
                channel: target.channel,
                note: target.note,
                velocity: command_u7(snapshot, command, "velocity", 0),
            }))
        }
        Some(crate::app::MidiSendFullNoteCommand::NODE_TYPE) => {
            let target = command_note_target(snapshot, command)?;
            let duration_ms = command_int(snapshot, command, "duration_ms", 100).max(0) as u64;
            Ok(MidiSendRequest {
                packets: vec![
                    MidiSendPacket::immediate(MidiMessage::NoteOn {
                        channel: target.channel,
                        note: target.note,
                        velocity: command_u7(snapshot, command, "velocity", DEFAULT_VELOCITY),
                    }),
                    MidiSendPacket::delayed(
                        MidiMessage::NoteOff {
                            channel: target.channel,
                            note: target.note,
                            velocity: command_u7(snapshot, command, "off_velocity", 0),
                        },
                        duration_ms,
                    ),
                ],
            })
        }
        Some(crate::app::MidiSendControlChangeCommand::NODE_TYPE) => {
            let channel = command_channel(snapshot, command);
            let controller = command_u7(snapshot, command, "controller", 1);
            if command_bool(snapshot, command, "is_14_bit", false) {
                let controller = validate_14_bit_controller(controller)?;
                let value = clamp_i32_to_u14(command_int(snapshot, command, "value_14_bit", 0));
                return Ok(MidiSendRequest::immediate_many(
                    encode_14_bit_control_change(channel, controller, value),
                    format!("14-bit cc ch{} {} value {}", channel, controller, value),
                ));
            }

            Ok(MidiSendRequest::immediate(MidiMessage::ControlChange {
                channel,
                controller,
                value: command_u7(snapshot, command, "value", 0),
            }))
        }
        Some(crate::app::MidiSendProgramChangeCommand::NODE_TYPE) => {
            Ok(MidiSendRequest::immediate(MidiMessage::ProgramChange {
                channel: command_channel(snapshot, command),
                program: command_u7(snapshot, command, "program", 0),
            }))
        }
        Some(crate::app::MidiSendPitchBendCommand::NODE_TYPE) => {
            Ok(MidiSendRequest::immediate(MidiMessage::PitchBend {
                channel: command_channel(snapshot, command),
                value: clamp_i32_to_u14(command_int(snapshot, command, "value", 8192)),
            }))
        }
        Some(crate::app::MidiSendChannelPressureCommand::NODE_TYPE) => {
            Ok(MidiSendRequest::immediate(MidiMessage::ChannelPressure {
                channel: command_channel(snapshot, command),
                pressure: command_u7(snapshot, command, "pressure", 0),
            }))
        }
        Some(crate::app::MidiSendPolyPressureCommand::NODE_TYPE) => {
            let target = command_note_target(snapshot, command)?;
            Ok(MidiSendRequest::immediate(MidiMessage::PolyPressure {
                channel: target.channel,
                note: target.note,
                pressure: command_u7(snapshot, command, "pressure", 0),
            }))
        }
        Some(crate::app::MidiSendSystemCommonCommand::NODE_TYPE) => {
            let message = command_enum(snapshot, command, "message", SYSTEM_COMMON_TUNE_REQUEST);
            let system = match message.as_str() {
                SYSTEM_COMMON_TIME_CODE_QUARTER_FRAME => MidiSystemMessage::TimeCodeQuarterFrame {
                    value: command_u7(snapshot, command, "quarter_frame", 0),
                },
                SYSTEM_COMMON_SONG_POSITION => MidiSystemMessage::SongPosition {
                    position: clamp_i32_to_u14(command_int(snapshot, command, "song_position", 0)),
                },
                SYSTEM_COMMON_SONG_SELECT => MidiSystemMessage::SongSelect {
                    song: command_u7(snapshot, command, "song", 0),
                },
                SYSTEM_COMMON_TUNE_REQUEST => MidiSystemMessage::TuneRequest,
                other => return Err(format!("unsupported system common message '{other}'")),
            };

            Ok(MidiSendRequest::immediate(MidiMessage::System(system)))
        }
        Some(crate::app::MidiSendSystemRealtimeCommand::NODE_TYPE) => {
            let message = command_enum(snapshot, command, "message", SYSTEM_REALTIME_START);
            let system = match message.as_str() {
                SYSTEM_REALTIME_TIMING_CLOCK => MidiSystemMessage::TimingClock,
                SYSTEM_REALTIME_START => MidiSystemMessage::Start,
                SYSTEM_REALTIME_CONTINUE => MidiSystemMessage::Continue,
                SYSTEM_REALTIME_STOP => MidiSystemMessage::Stop,
                SYSTEM_REALTIME_ACTIVE_SENSING => MidiSystemMessage::ActiveSensing,
                SYSTEM_REALTIME_RESET => MidiSystemMessage::Reset,
                other => return Err(format!("unsupported system realtime message '{other}'")),
            };

            Ok(MidiSendRequest::immediate(MidiMessage::System(system)))
        }
        Some(crate::app::MidiSendSysexBytesCommand::NODE_TYPE) => {
            let bytes = command_string(snapshot, command, "bytes").unwrap_or_default();
            Ok(MidiSendRequest {
                packets: vec![MidiSendPacket::immediate_bytes(
                    normalize_midi_sysex(parse_midi_bytes(&bytes)?.as_slice())?,
                    "sysex bytes",
                )],
            })
        }
        Some(crate::app::MidiSendSysexStringCommand::NODE_TYPE) => {
            let text = command_string(snapshot, command, "text").unwrap_or_default();
            Ok(MidiSendRequest {
                packets: vec![MidiSendPacket::immediate_bytes(
                    normalize_midi_sysex(text.as_bytes())?,
                    "sysex string",
                )],
            })
        }
        Some(crate::app::MidiSendRawBytesCommand::NODE_TYPE) => {
            let bytes = command_string(snapshot, command, "bytes").unwrap_or_default();
            Ok(MidiSendRequest {
                packets: vec![MidiSendPacket::immediate_bytes(
                    parse_midi_bytes(&bytes)?,
                    "raw MIDI bytes",
                )],
            })
        }
        _ => Err("Unsupported MIDI command type".into()),
    }
}

fn command_channel(snapshot: &View, command_id: NodeId) -> u8 {
    clamp_channel_i32(command_int(snapshot, command_id, "channel", DEFAULT_CHANNEL))
}

fn command_note_target(snapshot: &View, command_id: NodeId) -> Result<MidiNoteTarget, String> {
    Ok(MidiNoteTarget {
        channel: command_channel(snapshot, command_id),
        note: command_note_pitch(snapshot, command_id)?,
    })
}

fn command_note_pitch(snapshot: &View, command_id: NodeId) -> Result<u8, String> {
    let mode = command_enum(snapshot, command_id, "note_mode", NOTE_MODE_PITCH);
    if mode == NOTE_MODE_OCTAVE_NOTE {
        let note = command_enum(snapshot, command_id, "note", DEFAULT_NOTE_NAME);
        let octave = command_int(snapshot, command_id, "octave", DEFAULT_OCTAVE);
        return note_pitch_from_name_octave(note.as_str(), octave)
            .ok_or_else(|| format!("note {note}{octave} is outside the MIDI 0-127 pitch range"));
    }

    Ok(command_u7(snapshot, command_id, "pitch", DEFAULT_PITCH))
}

fn command_u7(snapshot: &View, command_id: NodeId, path: &str, fallback: i32) -> u8 {
    clamp_i32_to_u7(command_int(snapshot, command_id, path, fallback))
}

fn command_int(snapshot: &View, command_id: NodeId, path: &str, fallback: i32) -> i32 {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param_id| snapshot.value(param_id).and_then(ParamValue::as_int))
        .unwrap_or(fallback)
}

fn command_bool(snapshot: &View, command_id: NodeId, path: &str, fallback: bool) -> bool {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param_id| snapshot.value(param_id).and_then(ParamValue::as_bool))
        .unwrap_or(fallback)
}

fn command_enum(snapshot: &View, command_id: NodeId, path: &str, fallback: &str) -> String {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param_id| snapshot.value(param_id).and_then(ParamValue::as_enum))
        .unwrap_or_else(|| fallback.to_string())
}

fn command_string<'a>(snapshot: &'a View<'a>, command_id: NodeId, path: &str) -> Option<std::borrow::Cow<'a, str>> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot.value(param_id).map(|value| match value {
            ParamValue::Str(s) | ParamValue::File(s) | ParamValue::Enum(s) => std::borrow::Cow::Borrowed(s.as_str()),
            _ => std::borrow::Cow::Owned(value.as_str().unwrap_or_default()),
        })
    })
}
