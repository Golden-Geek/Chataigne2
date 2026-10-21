use super::*;
use crate::app::{append_midi_bytes, normalize_midi_sysex, MidiSendPacket};

#[derive(Clone, Copy)]
pub(super) enum Method {
    NoteOn,
    NoteOff,
    FullNote,
    Cc,
    Program,
    Bend,
    Pressure,
    PolyPressure,
    Sysex,
    Raw,
}
impl Method {
    pub(super) fn parse(method: &str) -> Option<Self> {
        Some(match method {
            "sendNoteOn" => Self::NoteOn,
            "sendNoteOff" => Self::NoteOff,
            "sendFullNote" => Self::FullNote,
            "sendCC" | "sendControlChange" => Self::Cc,
            "sendProgramChange" => Self::Program,
            "sendPitchBend" => Self::Bend,
            "sendChannelPressure" => Self::Pressure,
            "sendPolyPressure" => Self::PolyPressure,
            "sendSysEx" | "sendSysex" => Self::Sysex,
            "sendRawBytes" => Self::Raw,
            _ => return None,
        })
    }
}
pub(super) fn prepare(method: Method, args: &[ParamValue]) -> Result<MidiSendRequest, String> {
    let channel = || script_channel_arg(args, 0, "channel");
    let note = || script_u7_arg(args, 1, "note");
    let message = match method {
        Method::NoteOn => MidiMessage::NoteOn {
            channel: channel(),
            note: note(),
            velocity: script_u7_arg_or(args, 2, DEFAULT_SCRIPT_MIDI_VELOCITY),
        },
        Method::NoteOff => MidiMessage::NoteOff {
            channel: channel(),
            note: note(),
            velocity: script_u7_arg_or(args, 2, 0),
        },
        Method::FullNote => {
            return Ok(MidiSendRequest {
                packets: vec![
                    MidiSendPacket::immediate(MidiMessage::NoteOn {
                        channel: channel(),
                        note: note(),
                        velocity: script_u7_arg_or(args, 2, DEFAULT_SCRIPT_MIDI_VELOCITY),
                    }),
                    MidiSendPacket::delayed(
                        MidiMessage::NoteOff {
                            channel: channel(),
                            note: note(),
                            velocity: script_u7_arg_or(args, 4, 0),
                        },
                        script_nonnegative_u64_arg_or(args, 3, 100),
                    ),
                ],
            })
        }
        Method::Cc => MidiMessage::ControlChange {
            channel: channel(),
            controller: script_u7_arg(args, 1, "controller"),
            value: script_u7_arg(args, 2, "value"),
        },
        Method::Program => MidiMessage::ProgramChange {
            channel: channel(),
            program: script_u7_arg(args, 1, "program"),
        },
        Method::Bend => MidiMessage::PitchBend {
            channel: channel(),
            value: script_u14_arg(args, 1, "value"),
        },
        Method::Pressure => MidiMessage::ChannelPressure {
            channel: channel(),
            pressure: script_u7_arg(args, 1, "pressure"),
        },
        Method::PolyPressure => MidiMessage::PolyPressure {
            channel: channel(),
            note: note(),
            pressure: script_u7_arg(args, 2, "pressure"),
        },
        Method::Sysex | Method::Raw => {
            let mut bytes = Vec::new();
            for value in args {
                match value {
                    ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) => {
                        append_midi_bytes(&mut bytes, text)?
                    }
                    _ => append_midi_bytes(&mut bytes, &value.as_str().unwrap_or_default())?,
                }
            }
            let (bytes, description) = if matches!(method, Method::Sysex) {
                (normalize_midi_sysex(&bytes)?, "sysex bytes")
            } else {
                (bytes, "raw MIDI bytes")
            };
            return Ok(MidiSendRequest {
                packets: vec![MidiSendPacket::immediate_bytes(bytes, description)],
            });
        }
    };
    Ok(MidiSendRequest::immediate(message))
}
