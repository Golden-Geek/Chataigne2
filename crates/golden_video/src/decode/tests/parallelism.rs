use super::*;
use ffmpeg::codec::threading::Type;

fn assert_equal_frame(serial: &DecodedFrame, sliced: &DecodedFrame) {
    assert_eq!(serial.timestamp(), sliced.timestamp());
    let original = SoftwareImageConverter::default().convert(serial).unwrap();
    let parallel = SoftwareImageConverter::default().convert(sliced).unwrap();
    assert_eq!(original.descriptor(), parallel.descriptor());
    assert_eq!(original.pixels(), parallel.pixels());
}

#[test]
fn bounded_slice_modes_preserve_common_codec_pixels_timestamps_eof_and_seek() {
    for mode in [SoftwareDecodeThreads::Two, SoftwareDecodeThreads::Four] {
        for name in [
            "h264.mp4",
            "h264-color.mp4",
            "hevc.mp4",
            "hevc-main10.mp4",
            "prores.mov",
            "prores-alpha.mov",
        ] {
            let mut serial = Decoder::open(&fixture(name), limits()).unwrap();
            let mut sliced = Decoder::open(
                &fixture(name),
                DecodeLimits {
                    software_threads: mode,
                    ..limits()
                },
            )
            .unwrap();
            let active = sliced.decoder.as_ref().unwrap().threading();
            assert_ne!(active.kind, Type::Frame);
            let bound = match mode {
                SoftwareDecodeThreads::Two => 2,
                _ => 4,
            };
            assert!(active.count <= bound);
            if name.starts_with("prores") {
                assert_eq!(
                    active.kind,
                    Type::Slice,
                    "reference ProRes must use admitted slice workers"
                );
                assert_eq!(active.count, bound);
            }
            let mut timestamps = Vec::new();
            loop {
                match (serial.next_frame().unwrap(), sliced.next_frame().unwrap()) {
                    (Some(original), Some(parallel)) => {
                        assert_equal_frame(&original, &parallel);
                        timestamps.push(original.timestamp());
                    }
                    (None, None) => break,
                    _ => panic!("slice mode changed EOF for {name}"),
                }
            }
            assert_eq!(timestamps.len(), 6);
            for index in [4, 1, 5, 0] {
                serial.seek_before(timestamps[index]).unwrap();
                sliced.seek_before(timestamps[index]).unwrap();
                loop {
                    let original = serial.next_frame().unwrap().unwrap();
                    let parallel = sliced.next_frame().unwrap().unwrap();
                    assert_equal_frame(&original, &parallel);
                    if original.timestamp() >= timestamps[index] {
                        break;
                    }
                }
                assert!(sliced.software_decode_threads().unwrap() <= bound);
                assert_ne!(sliced.decoder.as_ref().unwrap().threading().kind, Type::Frame);
            }
        }
    }
}

#[test]
fn compressed_hap_never_allocates_a_codec_thread_pool() {
    for name in ["hap.mov", "hap-alpha.mov", "hap-q.mov"] {
        let mut decoder = Decoder::open(
            &fixture(name),
            DecodeLimits {
                software_threads: SoftwareDecodeThreads::Four,
                ..limits()
            },
        )
        .unwrap();
        assert_eq!(decoder.storage(), DecodeStorage::Hap);
        assert_eq!(decoder.software_decode_threads(), None);
        assert!(decoder.decoder.is_none());
        assert!(decoder.next_frame().unwrap().unwrap().hap_frame().is_some());
    }
}
