use super::super::Reader;
use golden_video::{
    Extent2D,
    decode::{DecodeLimits, Decoder, HapFormat, SoftwareDecodeThreads},
};
use std::time::Duration;

fn select(reader: &mut Reader, position: Duration, expected: &[u8]) {
    for _ in 0..16 {
        reader.advance(position).unwrap();
        if !reader.catching_up {
            let current = reader.current.as_ref().unwrap().hap_frame().unwrap();
            assert_eq!(current.blocks(), expected);
            assert_eq!(reader.neighbours().unwrap().unwrap().position, position);
            reader.mark_published();
            return;
        }
    }
    panic!("bounded HAP reader did not reach the requested timestamp");
}

#[test]
fn hap_reader_seeks_and_previous_probes_fit_three_blocks_while_an_upload_clone_is_held() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/golden_video/src/decode/tests/fixtures");
    for name in ["hap.mov", "hap-alpha.mov", "hap-q.mov"] {
        let open = || {
            Decoder::open(
                &root.join(name),
                DecodeLimits {
                    max_pixels: 4096 * 2160,
                    max_cpu_frame_bytes: 64 * 1024 * 1024,
                    max_packet_bytes: 16 * 1024 * 1024,
                    max_packets_per_frame: 32,
                    software_threads: SoftwareDecodeThreads::One,
                },
            )
            .unwrap()
        };
        let mut reader = Reader::new(open()).unwrap();
        let mut reference = open();
        let mut frames = Vec::new();
        while let Some(frame) = reference.next_frame().unwrap() {
            frames.push((
                reader.position(frame.timestamp()).unwrap(),
                frame.hap_frame().unwrap().blocks().to_vec(),
            ));
        }
        assert_eq!(frames.len(), 6);
        let info = reader.decoder.info();
        let block_bytes = HapFormat::Bc3Rgba
            .byte_len(Extent2D::new(info.width, info.height).unwrap())
            .unwrap() as u64;
        reader.decoder.tighten_cpu_frame_limit(3 * block_bytes).unwrap();
        let budget = reader.decoder.cpu_frame_budget().unwrap();
        select(&mut reader, frames[0].0, &frames[0].1);
        let upload = reader.current.as_ref().unwrap().hap_frame().unwrap().clone();
        for index in [5, 0, 4, 2, 1, 5] {
            select(&mut reader, frames[index].0, &frames[index].1);
            assert!(budget.snapshot().retained_bytes <= 3 * block_bytes);
            assert!(budget.snapshot().allocations <= 3);
            assert!(
                reader.converter.is_none(),
                "HAP never creates software conversion state"
            );
        }
        reader.previous = None;
        reader.prepare_previous().unwrap();
        select(&mut reader, frames[5].0, &frames[5].1);
        assert_eq!(reader.neighbours().unwrap().unwrap().previous, Some(frames[4].0));
        assert_eq!(upload.blocks(), frames[0].1);
        drop(reader);
        assert_eq!(budget.snapshot().retained_bytes, upload.blocks().len() as u64);
        drop(upload);
        assert_eq!(budget.snapshot().retained_bytes, 0);
        assert_eq!(budget.snapshot().allocations, 0);
    }
}
