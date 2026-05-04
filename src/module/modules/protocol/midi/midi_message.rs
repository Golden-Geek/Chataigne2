use midly::{
    live::{LiveEvent, MtcQuarterFrameMessage, SystemCommon, SystemRealtime},
    num::{u4, u7, u14},
    MidiMessage as MidlyMidiMessage, PitchBend,
};
use serde::{Deserialize, Serialize};

pub(crate) const MIDI_CHANNEL_MIN: u8 = 1;
pub(crate) const MIDI_CHANNEL_MAX: u8 = 16;
pub(crate) const MIDI_DATA_MAX: u8 = 127;
pub(crate) const MIDI_U14_MAX: u16 = 16_383;
pub(crate) const MIDI_PITCH_BEND_CENTER: u16 = 8_192;

pub(crate) const ROTARY_ABSOLUTE: &str = "absolute";
pub(crate) const ROTARY_TWOS_COMPLEMENT: &str = "twos_complement";
pub(crate) const ROTARY_BINARY_OFFSET: &str = "binary_offset";
pub(crate) const ROTARY_SIGN_MAGNITUDE: &str = "sign_magnitude";

const NOTE_NAMES_SHARP: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum MidiMessage {
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    PolyPressure {
        channel: u8,
        note: u8,
        pressure: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ChannelPressure {
        channel: u8,
        pressure: u8,
    },
    PitchBend {
        channel: u8,
        value: u16,
    },
    System(MidiSystemMessage),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum MidiSystemMessage {
    TimeCodeQuarterFrame { value: u8 },
    SongPosition { position: u16 },
    SongSelect { song: u8 },
    TuneRequest,
    TimingClock,
    Start,
    Continue,
    Stop,
    ActiveSensing,
    Reset,
    Sysex { bytes: Vec<u8> },
}

pub(crate) fn decode_midi_message(bytes: &[u8]) -> Option<MidiMessage> {
    match LiveEvent::parse(bytes).ok()? {
        LiveEvent::Midi { channel, message } => {
            let channel = channel.as_int() + 1;
            match message {
                MidlyMidiMessage::NoteOff { key, vel } => Some(MidiMessage::NoteOff {
                    channel,
                    note: key.as_int(),
                    velocity: vel.as_int(),
                }),
                MidlyMidiMessage::NoteOn { key, vel } => Some(MidiMessage::NoteOn {
                    channel,
                    note: key.as_int(),
                    velocity: vel.as_int(),
                }),
                MidlyMidiMessage::Aftertouch { key, vel } => Some(MidiMessage::PolyPressure {
                    channel,
                    note: key.as_int(),
                    pressure: vel.as_int(),
                }),
                MidlyMidiMessage::Controller { controller, value } => Some(MidiMessage::ControlChange {
                    channel,
                    controller: controller.as_int(),
                    value: value.as_int(),
                }),
                MidlyMidiMessage::ProgramChange { program } => Some(MidiMessage::ProgramChange {
                    channel,
                    program: program.as_int(),
                }),
                MidlyMidiMessage::ChannelAftertouch { vel } => Some(MidiMessage::ChannelPressure {
                    channel,
                    pressure: vel.as_int(),
                }),
                MidlyMidiMessage::PitchBend { bend } => Some(MidiMessage::PitchBend {
                    channel,
                    value: bend.0.as_int(),
                }),
            }
        }
        LiveEvent::Common(message) => decode_system_message(message),
        LiveEvent::Realtime(message) => decode_realtime_message(message),
    }
}

pub(crate) fn encode_midi_message(message: &MidiMessage) -> Vec<u8> {
    match message {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity,
        } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::NoteOff {
                key: midi_u7(*note),
                vel: midi_u7(*velocity),
            },
        }),
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::NoteOn {
                key: midi_u7(*note),
                vel: midi_u7(*velocity),
            },
        }),
        MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::Aftertouch {
                key: midi_u7(*note),
                vel: midi_u7(*pressure),
            },
        }),
        MidiMessage::ControlChange {
            channel,
            controller,
            value,
        } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::Controller {
                controller: midi_u7(*controller),
                value: midi_u7(*value),
            },
        }),
        MidiMessage::ProgramChange { channel, program } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::ProgramChange {
                program: midi_u7(*program),
            },
        }),
        MidiMessage::ChannelPressure { channel, pressure } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::ChannelAftertouch {
                vel: midi_u7(*pressure),
            },
        }),
        MidiMessage::PitchBend { channel, value } => write_live_event(LiveEvent::Midi {
            channel: midi_channel(*channel),
            message: MidlyMidiMessage::PitchBend {
                bend: PitchBend(u14::new((*value).min(MIDI_U14_MAX))),
            },
        }),
        MidiMessage::System(system) => encode_system_message(system),
    }
}

pub(crate) fn encode_14_bit_control_change(channel: u8, controller: u8, value: u16) -> Vec<MidiMessage> {
    let controller = controller.min(31);
    let (msb, lsb) = split_u14(value);
    vec![
        MidiMessage::ControlChange {
            channel,
            controller,
            value: msb,
        },
        MidiMessage::ControlChange {
            channel,
            controller: controller + 32,
            value: lsb,
        },
    ]
}

pub(crate) fn normalize_sysex_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut output = bytes.to_vec();
    if output.first().copied() != Some(0xF0) {
        output.insert(0, 0xF0);
    }
    if output.last().copied() != Some(0xF7) {
        output.push(0xF7);
    }
    output
}

pub(crate) fn note_label(note: u8) -> String {
    let note = note.min(MIDI_DATA_MAX);
    let name = NOTE_NAMES_SHARP[(note % 12) as usize];
    let octave = i16::from(note / 12) - 1;
    format!("{name}{octave}")
}

pub(crate) fn note_pitch_from_name_octave(note_name: &str, octave: i32) -> Option<u8> {
    let note_index = NOTE_NAMES_SHARP
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(note_name.trim()))?;
    let pitch = (octave + 1)
        .checked_mul(12)?
        .checked_add(i32::try_from(note_index).ok()?)?;
    u8::try_from(pitch).ok().filter(|value| *value <= MIDI_DATA_MAX)
}

pub(crate) fn channel_folder_label(channel: u8) -> String {
    format!("Channel {}", channel.clamp(MIDI_CHANNEL_MIN, MIDI_CHANNEL_MAX))
}

pub(crate) fn channel_decl_id(channel: u8) -> String {
    format!("channel_{}", channel.clamp(MIDI_CHANNEL_MIN, MIDI_CHANNEL_MAX))
}

pub(crate) fn note_decl_id(note: u8) -> String {
    format!("note_{}", note.min(MIDI_DATA_MAX))
}

pub(crate) fn cc_label(controller: u8) -> String {
    format!("CC {}", controller.min(MIDI_DATA_MAX))
}

pub(crate) fn cc_decl_id(controller: u8) -> String {
    format!("cc_{}", controller.min(MIDI_DATA_MAX))
}

pub(crate) fn decode_rotary_delta(mechanism: &str, raw_value: u8) -> Option<i32> {
    match mechanism {
        ROTARY_ABSOLUTE => None,
        ROTARY_TWOS_COMPLEMENT => Some(if raw_value <= 63 {
            i32::from(raw_value)
        } else {
            i32::from(raw_value) - 128
        }),
        ROTARY_BINARY_OFFSET => Some(i32::from(raw_value) - 64),
        ROTARY_SIGN_MAGNITUDE => {
            let magnitude = i32::from(raw_value & 0x3F);
            if raw_value & 0x40 == 0 {
                Some(magnitude)
            } else {
                Some(-magnitude)
            }
        }
        _ => None,
    }
}

pub(crate) fn encode_rotary_delta(mechanism: &str, delta: i32) -> Option<u8> {
    let delta = delta.clamp(-63, 63);
    match mechanism {
        ROTARY_ABSOLUTE => None,
        ROTARY_TWOS_COMPLEMENT => Some(if delta >= 0 {
            delta as u8
        } else {
            (128 + delta) as u8
        }),
        ROTARY_BINARY_OFFSET => Some((delta + 64) as u8),
        ROTARY_SIGN_MAGNITUDE => Some(if delta >= 0 {
            delta as u8
        } else {
            0x40 | (-delta as u8)
        }),
        _ => None,
    }
}

pub(crate) fn message_description(message: &MidiMessage) -> String {
    match message {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity,
        } => format!("note off ch{} {} velocity {}", channel, note_label(*note), velocity),
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        } => format!("note on ch{} {} velocity {}", channel, note_label(*note), velocity),
        MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        } => format!("poly pressure ch{} {} {}", channel, note_label(*note), pressure),
        MidiMessage::ControlChange {
            channel,
            controller,
            value,
        } => format!("cc ch{} {} value {}", channel, controller, value),
        MidiMessage::ProgramChange { channel, program } => {
            format!("program change ch{} {}", channel, program)
        }
        MidiMessage::ChannelPressure { channel, pressure } => {
            format!("channel pressure ch{} {}", channel, pressure)
        }
        MidiMessage::PitchBend { channel, value } => format!("pitch bend ch{} {}", channel, value),
        MidiMessage::System(system) => system_message_description(system),
    }
}

pub(crate) fn clamp_i32_to_u7(value: i32) -> u8 {
    value.clamp(0, i32::from(MIDI_DATA_MAX)) as u8
}

pub(crate) fn clamp_i32_to_u14(value: i32) -> u16 {
    value.clamp(0, i32::from(MIDI_U14_MAX)) as u16
}

pub(crate) fn clamp_channel_i32(value: i32) -> u8 {
    value.clamp(i32::from(MIDI_CHANNEL_MIN), i32::from(MIDI_CHANNEL_MAX)) as u8
}

fn decode_system_message(message: SystemCommon<'_>) -> Option<MidiMessage> {
    let message = match message {
        SystemCommon::SysEx(payload) => MidiSystemMessage::Sysex {
            bytes: sysex_bytes_from_payload(payload),
        },
        SystemCommon::MidiTimeCodeQuarterFrame(message, value) => MidiSystemMessage::TimeCodeQuarterFrame {
            value: encode_mtc_quarter_frame(message, value),
        },
        SystemCommon::SongPosition(position) => MidiSystemMessage::SongPosition {
            position: position.as_int(),
        },
        SystemCommon::SongSelect(song) => MidiSystemMessage::SongSelect {
            song: song.as_int(),
        },
        SystemCommon::TuneRequest => MidiSystemMessage::TuneRequest,
        SystemCommon::Undefined(_, _) => return None,
    };

    Some(MidiMessage::System(message))
}

fn decode_realtime_message(message: SystemRealtime) -> Option<MidiMessage> {
    let message = match message {
        SystemRealtime::TimingClock => MidiSystemMessage::TimingClock,
        SystemRealtime::Start => MidiSystemMessage::Start,
        SystemRealtime::Continue => MidiSystemMessage::Continue,
        SystemRealtime::Stop => MidiSystemMessage::Stop,
        SystemRealtime::ActiveSensing => MidiSystemMessage::ActiveSensing,
        SystemRealtime::Reset => MidiSystemMessage::Reset,
        SystemRealtime::Undefined(_) => return None,
    };

    Some(MidiMessage::System(message))
}

fn encode_system_message(message: &MidiSystemMessage) -> Vec<u8> {
    match message {
        MidiSystemMessage::TimeCodeQuarterFrame { value } => {
            let (message, nibble) = decode_mtc_quarter_frame(*value);
            write_live_event(LiveEvent::Common(SystemCommon::MidiTimeCodeQuarterFrame(
                message, nibble,
            )))
        }
        MidiSystemMessage::SongPosition { position } => write_live_event(LiveEvent::Common(
            SystemCommon::SongPosition(u14::new((*position).min(MIDI_U14_MAX))),
        )),
        MidiSystemMessage::SongSelect { song } => write_live_event(LiveEvent::Common(
            SystemCommon::SongSelect(midi_u7(*song)),
        )),
        MidiSystemMessage::TuneRequest => write_live_event(LiveEvent::Common(SystemCommon::TuneRequest)),
        MidiSystemMessage::TimingClock => write_live_event(LiveEvent::Realtime(SystemRealtime::TimingClock)),
        MidiSystemMessage::Start => write_live_event(LiveEvent::Realtime(SystemRealtime::Start)),
        MidiSystemMessage::Continue => write_live_event(LiveEvent::Realtime(SystemRealtime::Continue)),
        MidiSystemMessage::Stop => write_live_event(LiveEvent::Realtime(SystemRealtime::Stop)),
        MidiSystemMessage::ActiveSensing => write_live_event(LiveEvent::Realtime(SystemRealtime::ActiveSensing)),
        MidiSystemMessage::Reset => write_live_event(LiveEvent::Realtime(SystemRealtime::Reset)),
        MidiSystemMessage::Sysex { bytes } => {
            let payload = sysex_payload_bytes(bytes);
            write_live_event(LiveEvent::Common(SystemCommon::SysEx(u7::slice_from_int(
                payload.as_slice(),
            ))))
        }
    }
}

fn system_message_description(message: &MidiSystemMessage) -> String {
    match message {
        MidiSystemMessage::TimeCodeQuarterFrame { value } => {
            format!("time code quarter frame {}", value)
        }
        MidiSystemMessage::SongPosition { position } => format!("song position {}", position),
        MidiSystemMessage::SongSelect { song } => format!("song select {}", song),
        MidiSystemMessage::TuneRequest => "tune request".to_string(),
        MidiSystemMessage::TimingClock => "timing clock".to_string(),
        MidiSystemMessage::Start => "start".to_string(),
        MidiSystemMessage::Continue => "continue".to_string(),
        MidiSystemMessage::Stop => "stop".to_string(),
        MidiSystemMessage::ActiveSensing => "active sensing".to_string(),
        MidiSystemMessage::Reset => "reset".to_string(),
        MidiSystemMessage::Sysex { bytes } => format!("sysex {} byte(s)", bytes.len()),
    }
}

fn clamp_data(value: u8) -> u8 {
    value.min(MIDI_DATA_MAX)
}

fn midi_channel(channel: u8) -> u4 {
    u4::new(channel.clamp(MIDI_CHANNEL_MIN, MIDI_CHANNEL_MAX) - 1)
}

fn midi_u7(value: u8) -> u7 {
    u7::new(clamp_data(value))
}

fn write_live_event(event: LiveEvent<'_>) -> Vec<u8> {
    let mut bytes = Vec::new();
    event
        .write_std(&mut bytes)
        .expect("writing MIDI events into Vec<u8> should not fail");
    bytes
}

fn split_u14(value: u16) -> (u8, u8) {
    let value = value.min(MIDI_U14_MAX);
    (((value >> 7) & 0x7F) as u8, (value & 0x7F) as u8)
}

fn sysex_payload_bytes(bytes: &[u8]) -> Vec<u8> {
    let normalized = normalize_sysex_bytes(bytes);
    normalized[1..normalized.len() - 1]
        .iter()
        .copied()
        .map(clamp_data)
        .collect()
}

fn sysex_bytes_from_payload(payload: &[u7]) -> Vec<u8> {
    u7::slice_as_int(payload).to_vec()
}

fn encode_mtc_quarter_frame(message: MtcQuarterFrameMessage, value: u4) -> u8 {
    (mtc_quarter_frame_index(message) << 4) | value.as_int()
}

fn decode_mtc_quarter_frame(raw: u8) -> (MtcQuarterFrameMessage, u4) {
    (
        mtc_quarter_frame_message((raw >> 4) & 0x07),
        u4::new(raw & 0x0F),
    )
}

fn mtc_quarter_frame_index(message: MtcQuarterFrameMessage) -> u8 {
    match message {
        MtcQuarterFrameMessage::FramesLow => 0,
        MtcQuarterFrameMessage::FramesHigh => 1,
        MtcQuarterFrameMessage::SecondsLow => 2,
        MtcQuarterFrameMessage::SecondsHigh => 3,
        MtcQuarterFrameMessage::MinutesLow => 4,
        MtcQuarterFrameMessage::MinutesHigh => 5,
        MtcQuarterFrameMessage::HoursLow => 6,
        MtcQuarterFrameMessage::HoursHigh => 7,
    }
}

fn mtc_quarter_frame_message(index: u8) -> MtcQuarterFrameMessage {
    match index {
        0 => MtcQuarterFrameMessage::FramesLow,
        1 => MtcQuarterFrameMessage::FramesHigh,
        2 => MtcQuarterFrameMessage::SecondsLow,
        3 => MtcQuarterFrameMessage::SecondsHigh,
        4 => MtcQuarterFrameMessage::MinutesLow,
        5 => MtcQuarterFrameMessage::MinutesHigh,
        6 => MtcQuarterFrameMessage::HoursLow,
        7 => MtcQuarterFrameMessage::HoursHigh,
        _ => MtcQuarterFrameMessage::FramesLow,
    }
}

#[cfg(test)]
mod midi_message_tests;
