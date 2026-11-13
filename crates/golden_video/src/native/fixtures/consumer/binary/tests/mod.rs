use super::*;

fn spec(bits: u8) -> BinaryFrameCodeSpec {
    BinaryFrameCodeSpec {
        bits,
        origin: [24, 24],
        step: 24,
        sentinel: [4, 24],
        threshold: 128,
    }
}

fn words(bits: u8, code: u32) -> [u32; 12] {
    let mut words = [0; 12];
    words[..4].copy_from_slice(&[MAGIC, code, u32::MAX, u32::from(bits)]);
    for bit in 0..bits as usize {
        let value = if code & (1 << bit) != 0 { 235 } else { 16 };
        words[4 + bit / 4] |= value << ((bit % 4) * 8);
    }
    words
}

#[test]
fn binary_pixels_independently_decode_zero_twelve_and_full_width_codes() {
    for (bits, code) in [(1, 0), (1, 1), (12, 0), (12, 0xa5b), (32, u32::MAX), (32, 0x80000001)] {
        assert_eq!(spec(bits).decode(words(bits, code)), Ok(code));
    }
}

#[test]
fn bounded_coordinates_and_nonoverlapping_sentinel_are_required_before_gpu_creation() {
    let extent = crate::Extent2D::new(1920, 1080).unwrap();
    assert!(spec(32).validate(extent).is_ok());
    for bad in [
        BinaryFrameCodeSpec { bits: 0, ..spec(12) },
        BinaryFrameCodeSpec { bits: 33, ..spec(12) },
        BinaryFrameCodeSpec { step: 0, ..spec(12) },
        BinaryFrameCodeSpec {
            step: u32::MAX,
            ..spec(12)
        },
        BinaryFrameCodeSpec {
            origin: [u32::MAX, 0],
            ..spec(12)
        },
        BinaryFrameCodeSpec {
            sentinel: [1920, 0],
            ..spec(12)
        },
        BinaryFrameCodeSpec {
            sentinel: [24, 24],
            ..spec(12)
        },
        BinaryFrameCodeSpec {
            origin: [24, 1080],
            ..spec(12)
        },
        BinaryFrameCodeSpec {
            threshold: 0,
            ..spec(12)
        },
        BinaryFrameCodeSpec {
            threshold: 255,
            ..spec(12)
        },
    ] {
        assert_eq!(bad.validate(extent), Err(BinaryFrameCodeError::InvalidSpec));
    }
}

#[test]
fn corrupt_headers_sentinel_padding_and_gpu_code_never_become_valid_frames() {
    let marker = spec(12);
    for word in [0, 3, 11] {
        let mut bad = words(12, 60);
        bad[word] ^= 1;
        assert_eq!(marker.decode(bad), Err(BinaryFrameCodeError::Header));
    }
    let mut bad = words(12, 60);
    bad[2] = 0x00ffffff;
    assert_eq!(marker.decode(bad), Err(BinaryFrameCodeError::Sentinel));
    let mut bad = words(12, 60);
    bad[1] ^= 1;
    assert_eq!(marker.decode(bad), Err(BinaryFrameCodeError::DecodeMismatch));
}

#[test]
fn threshold_is_inclusive_and_shader_uses_the_declared_integer_coordinates() {
    let marker = spec(1);
    let mut raw = words(1, 1);
    raw[4] = 128;
    assert_eq!(marker.decode(raw), Ok(1));
    raw[4] = 127;
    assert_eq!(marker.decode(raw), Err(BinaryFrameCodeError::DecodeMismatch));
    let shader = marker.shader();
    assert!(shader.contains("#define CODE_BITS 1u"));
    assert!(shader.contains("#define CODE_X 24u"));
    assert!(shader.contains("#define SENTINEL_X 4u"));
}

#[test]
fn flattened_card_and_binary_results_have_distinct_truthful_json_fields() {
    use crate::native::fixtures::consumer::{CardMarkers, ConsumerFrame};
    let binary = ConsumerFrame {
        row: 0,
        token: crate::runtime::CompletionToken {
            device: crate::runtime::DeviceGeneration::new(uuid::Uuid::nil(), 0),
            serial: 1,
        },
        spout_image: None,
        receive_begin_qpc: None,
        completion_observed_qpc: None,
        raw_words: words(12, 60),
        marker: BinaryMarker {
            binary_frame_code: Ok(60),
        },
    };
    let encoded = serde_json::to_value(&binary).unwrap();
    assert_eq!(encoded["binary_frame_code"]["Ok"], 60);
    assert!(encoded.get("marker").is_none() && encoded.get("left").is_none());
    let card = ConsumerFrame {
        row: binary.row,
        token: binary.token,
        spout_image: None,
        receive_begin_qpc: None,
        completion_observed_qpc: None,
        raw_words: [0; 12],
        marker: CardMarkers {
            left: crate::native::fixtures::CardStamp::from_words([0; 6]),
            right: crate::native::fixtures::CardStamp::from_words([0; 6]),
        },
    };
    let encoded = serde_json::to_value(card).unwrap();
    assert!(encoded.get("left").is_some() && encoded.get("right").is_some());
    assert!(encoded.get("marker").is_none() && encoded.get("binary_frame_code").is_none());
}
