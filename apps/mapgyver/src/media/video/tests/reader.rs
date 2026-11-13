use super::*;
use golden_video::decode::{DecodeLimits, Decoder, SoftwareImageConverter};

#[test]
fn real_common_codecs_sample_exact_timestamps_rewind_after_eof_and_hold_shorter_endpoints() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/golden_video/src/decode/tests/fixtures");
    for name in ["h264.mp4", "hevc.mp4", "prores.mov"] {
        let open = || {
            Decoder::open(
                &root.join(name),
                DecodeLimits {
                    max_pixels: 4096 * 2160,
                    max_software_frame_bytes: 64 * 1024 * 1024,
                    max_packet_bytes: 1024 * 1024,
                    max_packets_per_frame: 256,
                    software_threads: golden_video::decode::SoftwareDecodeThreads::One,
                },
            )
            .unwrap()
        };
        let mut decoder = open();
        let base = decoder.info().time_base;
        let mut reference = Vec::new();
        let mut converter = SoftwareImageConverter::default();
        while let Some(frame) = decoder.next_frame().unwrap() {
            let time = Duration::from_nanos(
                u64::try_from(
                    (u128::try_from(frame.timestamp()).unwrap() * u128::from(base.numerator) * 1_000_000_000)
                        .div_ceil(u128::from(base.denominator)),
                )
                .unwrap(),
            );
            let image = converter.convert(&frame).unwrap();
            assert_eq!(image.descriptor().color_encoding, golden_video::ColorEncoding::Srgb);
            assert_eq!(image.pixels()[3], 255);
            reference.push((time, image.pixels()[..4].to_vec()));
        }
        assert_eq!(reference.len(), 6);
        let mut reader = super::super::reader::Reader::new(open()).unwrap();
        for index in [0, 5, 0, 4, 2, 1] {
            let mut pixels = None;
            for _ in 0..16 {
                if let Some(image) = reader.poll(reference[index].0).unwrap() {
                    pixels = Some(image);
                    break;
                }
            }
            assert_eq!(
                &pixels.unwrap().pixels()[..4],
                reference[index].1,
                "{name}: frame{index}"
            );
        }
        let last = reader.poll(Duration::from_secs(3600)).unwrap().unwrap();
        assert_eq!(&last.pixels()[..4], reference[5].1);
        assert!(reader.poll(Duration::from_secs(3600)).unwrap().is_none());
    }
}
