use super::*;

#[test]
fn binary_value_frames_round_trip_without_json() {
    let frame = ValueFrame {
        sequence: 7,
        generation: 3,
        samples: vec![
            ValueSample { slot: 1, value: 2.5 },
            ValueSample { slot: 9, value: -4.0 },
        ],
    };
    let bytes = frame.encode().unwrap();
    assert_eq!(&bytes[..4], b"GVF1");
    assert_eq!(ValueFrame::decode(&bytes, 16).unwrap(), frame);
}

#[test]
fn value_frame_limits_are_checked_before_sample_allocation() {
    let bytes = ValueFrame {
        sequence: 1,
        generation: 1,
        samples: vec![ValueSample { slot: 0, value: 1.0 }],
    }
    .encode()
    .unwrap();
    assert_eq!(
        ValueFrame::decode(&bytes, 0).unwrap_err(),
        ValueFrameError::SampleLimit { count: 1, maximum: 0 }
    );
}

#[test]
fn generated_types_include_every_protocol_plane() {
    let typescript = typescript_declarations();
    for name in [
        "ControlRequest",
        "AuthoringEvent",
        "ObservationInterest",
        "PreviewDelta",
        "ObservationMessage",
        "ServerMessage",
    ] {
        assert!(typescript.contains(name));
    }
}
