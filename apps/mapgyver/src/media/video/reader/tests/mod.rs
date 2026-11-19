#[cfg(all(windows, feature = "native-video"))]
use super::*;
mod hap;
#[cfg(all(windows, feature = "native-video"))]
mod webp_omission;

#[test]
#[cfg(all(windows, feature = "native-video"))]
fn animated_webp_forward_catch_up_progresses_and_held_seeks_remain_exact() {
    use golden_video::decode::{DecodeLimits, SoftwareDecodeThreads};
    use std::io::Write;

    // Seven independently encoded lossless 2×2 frames, each held for 200 ms.
    let encoded = concat!(
        "524946467201000057454250565038580a00000002000000010000010000414e494d0600000000000000",
        "0000414e4d4626000000000000000000010000010000c80000025650384c0e0000002f01400000071011",
        "fd0f4444ff03414e4d4628000000000000000000010000010000c80000005650384c0f0000002f014000",
        "000710f588fe0722a2ff0100414e4d4628000000000000000000010000010000c80000005650384c0f00",
        "00002f014000000710e589fe0722a2ff0100414e4d4628000000000000000000010000010000c8000000",
        "5650384c0f0000002f014000000710d58afe0722a2ff0100414e4d462800000000000000000001000001",
        "0000c80000005650384c0f0000002f014000000710c58bfe0722a2ff0100414e4d462800000000000000",
        "0000010000010000c80000005650384c0f0000002f014000000710b58cfe0722a2ff0100414e4d462800",
        "0000000000000000010000010000c80000005650384c0f0000002f014000000710a58dfe0722a2ff0100",
    );
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let mut file = tempfile::Builder::new().suffix(".webp").tempfile().unwrap();
    file.write_all(&bytes).unwrap();
    file.flush().unwrap();
    let open = || {
        Decoder::open(
            file.path(),
            DecodeLimits {
                max_pixels: 4096,
                max_cpu_frame_bytes: 16 * 1024 * 1024,
                max_packet_bytes: 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: SoftwareDecodeThreads::One,
            },
        )
        .unwrap()
    };
    let mut reader = Reader::new(open()).unwrap();
    let ms = Duration::from_millis;
    assert!(reader.advance_for_playback(ms(600), Some(ms(0))).unwrap());
    assert_eq!(reader.selected_neighbours().unwrap().unwrap().position, ms(0));
    assert!(reader.neighbours().unwrap().is_none());
    reader.mark_published();

    assert!(reader.advance_for_playback(ms(1200), Some(ms(0))).unwrap());
    let frame = reader.selected_neighbours().unwrap().unwrap();
    assert_eq!((frame.position, frame.next), (ms(200), Some(ms(400))));
    reader.mark_published();
    assert!(reader.advance_for_playback(ms(1200), Some(ms(0))).unwrap());
    let frame = reader.selected_neighbours().unwrap().unwrap();
    assert_eq!((frame.position, frame.next), (ms(400), Some(ms(600))));
    reader.mark_published();
    assert!(reader.advance(ms(1200)).unwrap());
    assert_eq!(reader.neighbours().unwrap().unwrap().position, ms(1200));
    assert!(reader.advance(ms(400)).unwrap());
    assert_eq!(reader.neighbours().unwrap().unwrap().position, ms(400));

    let mut cropped = Reader::new(open()).unwrap();
    assert!(!cropped.advance_for_playback(ms(800), Some(ms(400))).unwrap());
    assert!(!cropped.advance_for_playback(ms(800), Some(ms(400))).unwrap());
    assert!(cropped.advance_for_playback(ms(800), Some(ms(400))).unwrap());
    assert_eq!(cropped.selected_neighbours().unwrap().unwrap().position, ms(400));
}

#[test]
#[cfg(all(windows, feature = "native-video"))]
fn prepared_lookahead_is_reused_once_and_invalidated_by_seek_eof_and_previous_probe() {
    use golden_video::decode::{DecodeLimits, SoftwareDecodeThreads};
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/golden_video/src/decode/tests/fixtures");
    for name in ["h264.mp4", "hevc.mp4", "prores.mov", "prores-alpha.mov"] {
        let open = || {
            Decoder::open(
                &root.join(name),
                DecodeLimits {
                    max_pixels: 4096 * 2160,
                    max_cpu_frame_bytes: 64 * 1024 * 1024,
                    max_packet_bytes: 1024 * 1024,
                    max_packets_per_frame: 256,
                    software_threads: SoftwareDecodeThreads::One,
                },
            )
            .unwrap()
        };
        let mut baseline = Reader::new(open()).unwrap();
        let mut pipelined = Reader::new(open()).unwrap();
        let mut reference = open();
        let mut positions = Vec::new();
        while let Some(frame) = reference.next_frame().unwrap() {
            positions.push(baseline.position(frame.timestamp()).unwrap());
        }
        assert_eq!(positions.len(), 6);
        for (index, position) in positions.iter().copied().enumerate() {
            let expected = baseline.poll(position, None).unwrap().unwrap();
            assert!(pipelined.advance(position).unwrap());
            let image = pipelined.software_image().unwrap();
            assert_eq!(image.pixels(), expected.pixels(), "{name} frame{index}");
            assert!(pipelined.prepared.is_none(), "consumption removes the prepared owner");
            pipelined.mark_published();
            if let Some(frame) = &pipelined.next {
                let due = pipelined.position(frame.timestamp()).unwrap();
                pipelined.prepare_next(due).unwrap();
                assert!(
                    pipelined.prepared.is_none(),
                    "late sampling must not start speculative conversion"
                );
            }
            pipelined.prepare_next(position).unwrap();
            let prepared = pipelined
                .prepared
                .as_ref()
                .map(|(timestamp, image)| (*timestamp, image.pixels().as_ptr()));
            pipelined.prepare_next(position).unwrap();
            assert_eq!(
                pipelined
                    .prepared
                    .as_ref()
                    .map(|(timestamp, image)| (*timestamp, image.pixels().as_ptr())),
                prepared,
                "a held decoded lookahead must not allocate/convert again"
            );
            assert!(pipelined.poll(position, None).unwrap().is_none());
        }
        assert!(pipelined.prepared.is_none(), "EOF has no prepared future image");
        assert!(pipelined.advance(positions[0]).unwrap());
        assert!(pipelined.prepared.is_none(), "rewind invalidates cached future pixels");
        assert_eq!(
            pipelined.software_image().unwrap().pixels(),
            baseline.poll(positions[0], None).unwrap().unwrap().pixels()
        );
        pipelined.prepare_next(positions[0]).unwrap();
        assert!(pipelined.prepared.is_some());
        assert!(pipelined.advance(positions[4]).unwrap());
        assert_eq!(
            pipelined.software_image().unwrap().pixels(),
            baseline.poll(positions[4], None).unwrap().unwrap().pixels(),
            "a skipped cached frame cannot replace the selected timestamp"
        );
        pipelined.prepare_next(positions[4]).unwrap();
        // An explicit paused predecessor probe resets decoder-owned lookahead and prepared pixels.
        pipelined.previous = None;
        pipelined.prepare_previous().unwrap();
        assert!(pipelined.prepared.is_none());
    }
}
