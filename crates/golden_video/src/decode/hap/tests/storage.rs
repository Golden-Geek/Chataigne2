use super::*;
use crate::decode::{DecodeLimits, Decoder, SoftwareDecodeThreads};

#[test]
fn raw_snappy_and_odd_blocks_share_one_charge_through_clones_and_restore_capacity_on_release() {
    for (tag, format, extent) in [
        (0xAB, HapFormat::Bc1Rgb, Extent2D::new(4, 4).unwrap()),
        (0xAE, HapFormat::Bc3Rgba, Extent2D::new(17, 9).unwrap()),
        (0xAF, HapFormat::Bc3YCoCg, Extent2D::new(65, 7).unwrap()),
    ] {
        let bytes = format.byte_len(extent).unwrap();
        let budget = ImageStorageBudget::new(2 * bytes as u64).unwrap();
        let original = vec![17; bytes];
        let raw = section(tag, &original);
        let compressed = section(
            tag + 0x10,
            &snap::raw::Encoder::new().compress_vec(&vec![29; bytes]).unwrap(),
        );
        let frame = HapFrame::decode(&raw, extent, &budget).unwrap();
        let held = frame.clone();
        assert_eq!(held.blocks().as_ptr(), frame.blocks().as_ptr());
        assert_eq!(budget.snapshot().allocations, 1, "clones share one payload reservation");
        let second = HapFrame::decode(&compressed, extent, &budget).unwrap();
        let full = budget.snapshot();
        assert_eq!(full.retained_bytes, 2 * bytes as u64);
        assert!(matches!(
            HapFrame::decode(&raw, extent, &budget),
            Err(DecodeError::LimitExceeded)
        ));
        assert_eq!(
            budget.snapshot(),
            full,
            "capacity rejection cannot allocate new block storage"
        );
        drop(frame);
        assert_eq!(
            budget.snapshot(),
            full,
            "the upload clone still pins its original blocks"
        );
        drop(second);
        let resumed = HapFrame::decode(&compressed, extent, &budget).unwrap();
        assert_eq!(held.blocks(), original);
        assert_eq!(resumed.blocks(), vec![29; bytes]);
        drop(held);
        assert_eq!(budget.snapshot().retained_bytes, bytes as u64);
        drop(resumed);
        assert_eq!(budget.snapshot().retained_bytes, 0);
        assert_eq!(budget.snapshot().allocations, 0);
    }
}

#[test]
fn malformed_decode_releases_its_reservation_and_insufficient_storage_rejects_before_allocation() {
    let extent = Extent2D::new(4, 4).unwrap();
    let tiny = ImageStorageBudget::new(7).unwrap();
    assert!(matches!(
        HapFrame::decode(&section(0xAB, &[1; 8]), extent, &tiny),
        Err(DecodeError::LimitExceeded)
    ));
    assert_eq!(tiny.snapshot().peak_bytes, 0);
    let budget = ImageStorageBudget::new(8).unwrap();
    assert!(HapFrame::decode(&section(0xAB, &[1; 7]), extent, &budget).is_err());
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(budget.snapshot().allocations, 0);
    let healthy = HapFrame::decode(&section(0xAB, &[9; 8]), extent, &budget).unwrap();
    assert_eq!(healthy.blocks(), &[9; 8]);
    drop(healthy);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}

fn limits(bytes: u64) -> DecodeLimits {
    DecodeLimits {
        max_pixels: 4096 * 2160,
        max_cpu_frame_bytes: bytes,
        max_packet_bytes: 16 * 1024 * 1024,
        max_packets_per_frame: 32,
        software_threads: SoftwareDecodeThreads::One,
    }
}
fn path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/decode/tests/fixtures")
        .join(name)
}

#[test]
fn all_hap_decoder_variants_keep_one_budget_across_eof_seek_and_final_clone_retirement() {
    for name in ["hap.mov", "hap-alpha.mov", "hap-q.mov"] {
        let mut decoder = Decoder::open(&path(name), limits(64 * 1024 * 1024)).unwrap();
        let budget = decoder.cpu_frame_budget().unwrap();
        let first = decoder.next_frame().unwrap().unwrap();
        let held = first.hap_frame().unwrap().clone();
        let original = held.blocks().to_vec();
        let bytes = original.len() as u64;
        drop(first);
        decoder.tighten_cpu_frame_limit(3 * bytes).unwrap();
        while decoder.next_frame().unwrap().is_some() {}
        assert_eq!(budget.snapshot().retained_bytes, bytes);
        assert!(budget.snapshot().peak_bytes <= 3 * bytes);
        decoder.seek_before(decoder.info().start.unwrap_or(0)).unwrap();
        let again = decoder.next_frame().unwrap().unwrap();
        assert_eq!(again.hap_frame().unwrap().blocks(), original);
        assert_eq!(budget.snapshot(), decoder.cpu_frame_budget().unwrap().snapshot());
        drop(again);
        drop(decoder);
        assert_eq!(budget.snapshot().retained_bytes, bytes);
        assert_eq!(held.blocks(), original);
        drop(held);
        assert_eq!(budget.snapshot().retained_bytes, 0);
        assert_eq!(budget.snapshot().allocations, 0);
    }
}

#[test]
fn exhausted_hap_decoder_is_poisoned_without_freeing_the_retained_original() {
    let mut decoder = Decoder::open(&path("hap.mov"), limits(64 * 1024 * 1024)).unwrap();
    let first = decoder.next_frame().unwrap().unwrap();
    let budget = decoder.cpu_frame_budget().unwrap();
    decoder
        .tighten_cpu_frame_limit(first.hap_frame().unwrap().blocks().len() as u64)
        .unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::LimitExceeded)));
    assert!(matches!(decoder.next_frame(), Err(DecodeError::ReopenRequired)));
    assert!(matches!(decoder.seek_before(0), Err(DecodeError::ReopenRequired)));
    drop(decoder);
    assert!(budget.snapshot().retained_bytes > 0);
    drop(first);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}
