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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MidiChannelVoiceStatus {
    pub channel: u8,
    pub kind: MidiChannelVoiceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MidiChannelVoiceKind {
    NoteOff,
    NoteOn,
    PolyPressure,
    ControlChange,
    ProgramChange,
    ChannelPressure,
    PitchBend,
}

pub(crate) fn decode_midi_message(bytes: &[u8]) -> Option<MidiMessage> {
    let (&status, payload) = bytes.split_first()?;
    if status == 0xF0 {
        return Some(MidiMessage::System(MidiSystemMessage::Sysex {
            bytes: normalize_sysex_bytes(bytes),
        }));
    }

    if status >= 0xF0 {
        return decode_system_message(status, payload);
    }

    let voice = decode_channel_voice_status(status)?;
    match voice.kind {
        MidiChannelVoiceKind::NoteOff => Some(MidiMessage::NoteOff {
            channel: voice.channel,
            note: data_byte(payload.first().copied()?)?,
            velocity: data_byte(payload.get(1).copied()?)?,
        }),
        MidiChannelVoiceKind::NoteOn => Some(MidiMessage::NoteOn {
            channel: voice.channel,
            note: data_byte(payload.first().copied()?)?,
            velocity: data_byte(payload.get(1).copied()?)?,
        }),
        MidiChannelVoiceKind::PolyPressure => Some(MidiMessage::PolyPressure {
            channel: voice.channel,
            note: data_byte(payload.first().copied()?)?,
            pressure: data_byte(payload.get(1).copied()?)?,
        }),
        MidiChannelVoiceKind::ControlChange => Some(MidiMessage::ControlChange {
            channel: voice.channel,
            controller: data_byte(payload.first().copied()?)?,
            value: data_byte(payload.get(1).copied()?)?,
        }),
        MidiChannelVoiceKind::ProgramChange => Some(MidiMessage::ProgramChange {
            channel: voice.channel,
            program: data_byte(payload.first().copied()?)?,
        }),
        MidiChannelVoiceKind::ChannelPressure => Some(MidiMessage::ChannelPressure {
            channel: voice.channel,
            pressure: data_byte(payload.first().copied()?)?,
        }),
        MidiChannelVoiceKind::PitchBend => {
            let lsb = data_byte(payload.first().copied()?)?;
            let msb = data_byte(payload.get(1).copied()?)?;
            Some(MidiMessage::PitchBend {
                channel: voice.channel,
                value: u14_from_msb_lsb(msb, lsb),
            })
        }
    }
}

pub(crate) fn encode_midi_message(message: &MidiMessage) -> Vec<u8> {
    match message {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity,
        } => vec![status_byte(0x80, *channel), clamp_data(*note), clamp_data(*velocity)],
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        } => vec![status_byte(0x90, *channel), clamp_data(*note), clamp_data(*velocity)],
        MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        } => vec![status_byte(0xA0, *channel), clamp_data(*note), clamp_data(*pressure)],
        MidiMessage::ControlChange {
            channel,
            controller,
            value,
        } => vec![
            status_byte(0xB0, *channel),
            clamp_data(*controller),
            clamp_data(*value),
        ],
        MidiMessage::ProgramChange { channel, program } => {
            vec![status_byte(0xC0, *channel), clamp_data(*program)]
        }
        MidiMessage::ChannelPressure { channel, pressure } => {
            vec![status_byte(0xD0, *channel), clamp_data(*pressure)]
        }
        MidiMessage::PitchBend { channel, value } => {
            let (msb, lsb) = split_u14(*value);
            vec![status_byte(0xE0, *channel), lsb, msb]
        }
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

pub(crate) fn note_name_for_pitch(note: u8) -> &'static str {
    NOTE_NAMES_SHARP[(note.min(MIDI_DATA_MAX) % 12) as usize]
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

fn decode_channel_voice_status(status: u8) -> Option<MidiChannelVoiceStatus> {
    let channel = (status & 0x0F) + 1;
    let kind = match status & 0xF0 {
        0x80 => MidiChannelVoiceKind::NoteOff,
        0x90 => MidiChannelVoiceKind::NoteOn,
        0xA0 => MidiChannelVoiceKind::PolyPressure,
        0xB0 => MidiChannelVoiceKind::ControlChange,
        0xC0 => MidiChannelVoiceKind::ProgramChange,
        0xD0 => MidiChannelVoiceKind::ChannelPressure,
        0xE0 => MidiChannelVoiceKind::PitchBend,
        _ => return None,
    };
    Some(MidiChannelVoiceStatus { channel, kind })
}

fn decode_system_message(status: u8, payload: &[u8]) -> Option<MidiMessage> {
    let message = match status {
        0xF1 => MidiSystemMessage::TimeCodeQuarterFrame {
            value: data_byte(payload.first().copied()?)?,
        },
        0xF2 => {
            let lsb = data_byte(payload.first().copied()?)?;
            let msb = data_byte(payload.get(1).copied()?)?;
            MidiSystemMessage::SongPosition {
                position: u14_from_msb_lsb(msb, lsb),
            }
        }
        0xF3 => MidiSystemMessage::SongSelect {
            song: data_byte(payload.first().copied()?)?,
        },
        0xF6 => MidiSystemMessage::TuneRequest,
        0xF8 => MidiSystemMessage::TimingClock,
        0xFA => MidiSystemMessage::Start,
        0xFB => MidiSystemMessage::Continue,
        0xFC => MidiSystemMessage::Stop,
        0xFE => MidiSystemMessage::ActiveSensing,
        0xFF => MidiSystemMessage::Reset,
        _ => return None,
    };
    Some(MidiMessage::System(message))
}

fn encode_system_message(message: &MidiSystemMessage) -> Vec<u8> {
    match message {
        MidiSystemMessage::TimeCodeQuarterFrame { value } => vec![0xF1, clamp_data(*value)],
        MidiSystemMessage::SongPosition { position } => {
            let (msb, lsb) = split_u14(*position);
            vec![0xF2, lsb, msb]
        }
        MidiSystemMessage::SongSelect { song } => vec![0xF3, clamp_data(*song)],
        MidiSystemMessage::TuneRequest => vec![0xF6],
        MidiSystemMessage::TimingClock => vec![0xF8],
        MidiSystemMessage::Start => vec![0xFA],
        MidiSystemMessage::Continue => vec![0xFB],
        MidiSystemMessage::Stop => vec![0xFC],
        MidiSystemMessage::ActiveSensing => vec![0xFE],
        MidiSystemMessage::Reset => vec![0xFF],
        MidiSystemMessage::Sysex { bytes } => normalize_sysex_bytes(bytes),
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

fn data_byte(value: u8) -> Option<u8> {
    (value <= MIDI_DATA_MAX).then_some(value)
}

fn clamp_data(value: u8) -> u8 {
    value.min(MIDI_DATA_MAX)
}

fn status_byte(base: u8, channel: u8) -> u8 {
    base | (channel.clamp(MIDI_CHANNEL_MIN, MIDI_CHANNEL_MAX) - 1)
}

fn split_u14(value: u16) -> (u8, u8) {
    let value = value.min(MIDI_U14_MAX);
    (((value >> 7) & 0x7F) as u8, (value & 0x7F) as u8)
}

fn u14_from_msb_lsb(msb: u8, lsb: u8) -> u16 {
    (u16::from(msb.min(MIDI_DATA_MAX)) << 7) | u16::from(lsb.min(MIDI_DATA_MAX))
}
