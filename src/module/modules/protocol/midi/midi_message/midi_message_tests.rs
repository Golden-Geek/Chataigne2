use super::{
    MidiMessage, MidiSystemMessage, cc_supports_14_bit, decode_midi_message, encode_14_bit_control_change,
    encode_midi_message, normalize_sysex_bytes,
};

#[test]
fn note_on_round_trips_through_midly_codec() {
    let message = MidiMessage::NoteOn {
        channel: 3,
        note: 64,
        velocity: 96,
    };

    let bytes = encode_midi_message(&message);

    assert_eq!(bytes, vec![0x92, 0x40, 0x60]);
    assert_eq!(decode_midi_message(bytes.as_slice()), Some(message));
}

#[test]
fn pitch_bend_round_trips_through_midly_codec() {
    let message = MidiMessage::PitchBend {
        channel: 10,
        value: 12_345,
    };

    let bytes = encode_midi_message(&message);

    assert_eq!(decode_midi_message(bytes.as_slice()), Some(message));
}

#[test]
fn sysex_round_trips_with_wrappers() {
    let message = MidiMessage::System(MidiSystemMessage::Sysex {
        bytes: vec![0x7D, 0x10, 0x20],
    });

    let bytes = encode_midi_message(&message);

    assert_eq!(bytes, normalize_sysex_bytes(&[0x7D, 0x10, 0x20]));
    assert_eq!(decode_midi_message(bytes.as_slice()), Some(message));
}

#[test]
fn quarter_frame_round_trips() {
    let message = MidiMessage::System(MidiSystemMessage::TimeCodeQuarterFrame { value: 0x73 });

    let bytes = encode_midi_message(&message);

    assert_eq!(bytes, vec![0xF1, 0x73]);
    assert_eq!(decode_midi_message(bytes.as_slice()), Some(message));
}

#[test]
fn fourteen_bit_control_change_uses_lsb_controller_plus_32() {
    let messages = encode_14_bit_control_change(2, 7, 0x2345);

    assert_eq!(
        messages,
        vec![
            MidiMessage::ControlChange {
                channel: 2,
                controller: 7,
                value: 0x46,
            },
            MidiMessage::ControlChange {
                channel: 2,
                controller: 39,
                value: 0x45,
            },
        ]
    );
    assert!(cc_supports_14_bit(7));
    assert!(!cc_supports_14_bit(39));
}
