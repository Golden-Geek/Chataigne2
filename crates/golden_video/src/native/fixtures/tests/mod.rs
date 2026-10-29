use super::*;

mod copy_lineage;
mod shader;

#[test]
fn image_marker_round_trips_full_width_identity_frame_and_clock() {
    for stamp in [
        CardStamp {
            source: 0,
            frame: 0,
            qpc: 1,
        },
        CardStamp {
            source: u32::MAX,
            frame: u64::MAX,
            qpc: u64::MAX,
        },
        CardStamp {
            source: 0x1234_5678,
            frame: 0x9abc_def0_1357_2468,
            qpc: 0xfedc_ba98_7654_3210,
        },
    ] {
        let cells = encode_marker(stamp);
        assert!(cells.iter().all(|cell| *cell <= 0xfff));
        assert_eq!(decode_marker(cells), Ok(stamp));
    }
}

#[test]
fn every_single_corrupt_marker_bit_is_rejected() {
    let stamp = CardStamp {
        source: 7,
        frame: 0x1234_5678_9abc_def0,
        qpc: 0x1357_2468_0fed_cba9,
    };
    let original = encode_marker(stamp);
    for bit in 0..192 {
        let mut corrupt = original;
        corrupt[bit / 12] ^= 1 << (bit % 12);
        assert_eq!(decode_marker(corrupt), Err(MarkerError::Checksum));
    }
    let mut invalid = original;
    invalid[3] |= 0x1000;
    assert_eq!(decode_marker(invalid), Err(MarkerError::InvalidCell { index: 3 }));
}

#[test]
fn opaque_rgb_nibbles_survive_eight_bit_quantization() {
    for cell in 0..=0xfff_u16 {
        let quantized =
            [cell & 15, (cell >> 4) & 15, (cell >> 8) & 15].map(|nibble| (u32::from(nibble) * 255 / 15) as u8);
        let nibbles = quantized.map(|byte| ((u32::from(byte) * 15 + 127) / 255) as u16);
        assert_eq!(nibbles[0] | (nibbles[1] << 4) | (nibbles[2] << 8), cell);
    }
}
