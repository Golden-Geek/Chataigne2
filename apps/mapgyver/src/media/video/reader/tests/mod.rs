#[cfg(all(windows, feature = "native-video"))]
use super::*;

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
            let expected = baseline.poll(position).unwrap().unwrap();
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
            assert!(pipelined.poll(position).unwrap().is_none());
        }
        assert!(pipelined.prepared.is_none(), "EOF has no prepared future image");
        assert!(pipelined.advance(positions[0]).unwrap());
        assert!(pipelined.prepared.is_none(), "rewind invalidates cached future pixels");
        assert_eq!(
            pipelined.software_image().unwrap().pixels(),
            baseline.poll(positions[0]).unwrap().unwrap().pixels()
        );
        pipelined.prepare_next(positions[0]).unwrap();
        assert!(pipelined.prepared.is_some());
        assert!(pipelined.advance(positions[4]).unwrap());
        assert_eq!(
            pipelined.software_image().unwrap().pixels(),
            baseline.poll(positions[4]).unwrap().unwrap().pixels(),
            "a skipped cached frame cannot replace the selected timestamp"
        );
        pipelined.prepare_next(positions[4]).unwrap();
        // An explicit paused predecessor probe resets decoder-owned lookahead and prepared pixels.
        pipelined.previous = None;
        pipelined.prepare_previous().unwrap();
        assert!(pipelined.prepared.is_none());
    }
}
