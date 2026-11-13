use super::*;
use std::collections::HashMap;

fn buffers(frame: &DecodedFrame) -> HashMap<usize, u64> {
    // SAFETY: immutable metadata on this retained software frame's buffer references.
    let image = unsafe { &*frame.software_image().unwrap().as_ptr() };
    image
        .buf
        .iter()
        .filter(|buffer| !buffer.is_null())
        .map(|buffer| unsafe { ((**buffer).data as usize, (**buffer).size as u64) })
        .collect()
}

#[test]
fn actual_plane_payloads_include_idle_pool_storage_and_outlive_the_decoder() {
    for name in [
        "h264.mp4",
        "h264-color.mp4",
        "hevc.mp4",
        "hevc-main10.mp4",
        "prores.mov",
        "prores-odd.mov",
        "prores-alpha.mov",
    ] {
        let mut decoder = Decoder::open(&fixture(name), limits()).unwrap();
        let budget = decoder.cpu_frame_budget().unwrap();
        assert_eq!(budget.snapshot().retained_bytes, 0);
        let retained = decoder.next_frame().unwrap().unwrap();
        let original_pixels = retained.software_image().unwrap().data(0).to_vec();
        let retained_payload = buffers(&retained);
        let mut observed = retained_payload.clone();
        while let Some(frame) = decoder.next_frame().unwrap() {
            observed.extend(buffers(&frame));
            assert!(budget.snapshot().retained_bytes <= budget.snapshot().limit_bytes);
        }
        let allocated = budget.snapshot();
        assert_eq!(allocated.retained_bytes, observed.values().sum::<u64>(), "{name}");
        assert_eq!(allocated.allocations, observed.len() as u64, "{name}");
        assert_eq!(allocated.peak_bytes, allocated.retained_bytes);
        assert!(allocated.retained_bytes > retained_payload.values().sum::<u64>());
        drop(decoder);
        assert_eq!(budget.snapshot().retained_bytes, retained_payload.values().sum::<u64>());
        assert_eq!(budget.snapshot().allocations, retained_payload.len() as u64);
        assert_eq!(retained.software_image().unwrap().data(0), original_pixels);
        drop(retained);
        assert_eq!(budget.snapshot().retained_bytes, 0, "{name}");
        assert_eq!(budget.snapshot().allocations, 0, "{name}");
    }
}

#[test]
fn allocation_rejection_is_typed_precedes_payload_and_poisoned_owner_requires_reopen() {
    assert!(matches!(
        Decoder::open(
            &fixture("prores.mov"),
            DecodeLimits {
                max_cpu_frame_bytes: 0,
                ..limits()
            }
        ),
        Err(DecodeError::InvalidLimits)
    ));
    let mut decoder = Decoder::open(&fixture("prores.mov"), limits()).unwrap();
    let budget = decoder.cpu_frame_budget().unwrap();
    decoder.tighten_cpu_frame_limit(1).unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::LimitExceeded)));
    assert_eq!(
        budget.snapshot().peak_bytes,
        0,
        "a rejected buffer must not allocate payload"
    );
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert!(matches!(decoder.next_frame(), Err(DecodeError::ReopenRequired)));
    assert!(matches!(decoder.seek_before(0), Err(DecodeError::ReopenRequired)));
    assert!(matches!(
        decoder.tighten_cpu_frame_limit(1),
        Err(DecodeError::ReopenRequired)
    ));
}

#[test]
fn tightening_and_codec_replacement_preserve_the_same_pool_without_invalidating_frames() {
    let mut decoder = Decoder::open(&fixture("prores.mov"), limits()).unwrap();
    let budget = decoder.cpu_frame_budget().unwrap();
    let retained = decoder.next_frame().unwrap().unwrap();
    let pixels = retained.software_image().unwrap().data(0).to_vec();
    while decoder.next_frame().unwrap().is_some() {}
    let allocated = budget.snapshot();
    assert!(matches!(
        decoder.tighten_cpu_frame_limit(0),
        Err(DecodeError::InvalidLimits)
    ));
    assert!(matches!(
        decoder.tighten_cpu_frame_limit(allocated.limit_bytes + 1),
        Err(DecodeError::InvalidLimits)
    ));
    assert!(matches!(
        decoder.tighten_cpu_frame_limit(allocated.retained_bytes - 1),
        Err(DecodeError::LimitExceeded)
    ));
    assert_eq!(budget.snapshot(), allocated, "failed tightening is atomic");
    decoder.tighten_cpu_frame_limit(allocated.retained_bytes).unwrap();
    for _ in 0..8 {
        decoder.seek_before(0).unwrap();
        assert_eq!(decoder.next_frame().unwrap().unwrap().timestamp(), retained.timestamp());
        while decoder.next_frame().unwrap().is_some() {}
        assert_eq!(budget.snapshot().retained_bytes, allocated.retained_bytes);
        assert_eq!(budget.snapshot().allocations, allocated.allocations);
        assert_eq!(budget.snapshot(), decoder.cpu_frame_budget().unwrap().snapshot());
        assert_eq!(retained.software_image().unwrap().data(0), pixels);
    }
    drop(decoder);
    assert!(budget.snapshot().retained_bytes > 0);
    drop(retained);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}

#[test]
fn partial_plane_failure_keeps_allocated_cached_bytes_charged_until_pool_destruction() {
    let mut baseline = Decoder::open(&fixture("prores.mov"), limits()).unwrap();
    let first = baseline.next_frame().unwrap().unwrap();
    let bytes = buffers(&first).values().sum::<u64>();
    let mut decoder = Decoder::open(&fixture("prores.mov"), limits()).unwrap();
    decoder.tighten_cpu_frame_limit(bytes - 1).unwrap();
    let budget = decoder.cpu_frame_budget().unwrap();
    assert!(matches!(decoder.next_frame(), Err(DecodeError::LimitExceeded)));
    let partial = budget.snapshot();
    assert!(partial.retained_bytes > 0 && partial.retained_bytes < bytes);
    assert_eq!(partial.retained_bytes, partial.peak_bytes);
    drop(decoder);
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(budget.snapshot().allocations, 0);
}
