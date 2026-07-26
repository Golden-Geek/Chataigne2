use std::time::Duration;

use allocation_counter::measure;

use super::{ReferenceWorkload, ReferenceWorkloadHarness};

mod device_soak;

#[test]
fn small_reference_workload_is_finite_active_and_allocation_free_after_warmup() {
    let mut harness = ReferenceWorkloadHarness::new(ReferenceWorkload::Small).unwrap();
    harness.render_blocks(8).unwrap();
    let allocation = measure(|| harness.render_blocks(64).unwrap());
    assert_eq!(allocation.count_total, 0, "{allocation:?}");
    assert_eq!(allocation.count_current, 0, "{allocation:?}");
    assert_eq!(allocation.bytes_total, 0, "{allocation:?}");
    assert_eq!(allocation.bytes_current, 0, "{allocation:?}");

    let observation = harness.observation();
    assert!(observation.finite_output);
    assert!(observation.peak_output > 0.0);
    assert_eq!(observation.active_voices, 4);
    assert_eq!(observation.rendered_frames, 72 * 128);
    assert_eq!(observation.specification.routes, 16);
}

#[test]
fn medium_reference_workload_produces_meter_pitch_and_spectrum_evidence() {
    let mut harness = ReferenceWorkloadHarness::new(ReferenceWorkload::Medium).unwrap();
    harness.render_blocks(96).unwrap();
    assert!(harness.wait_for_analysis(Duration::from_secs(2)));

    let observation = harness.observation();
    assert!(observation.finite_output);
    assert!(observation.input_global_max_rms > 0.0);
    assert!(observation.output_global_max_rms > 0.0);
    assert_eq!(observation.observed_pitch_taps, 1);
    assert_eq!(observation.observed_spectrum_taps, 1);
    assert_eq!(observation.active_voices, 32);
    assert!(observation.estimated_resident_bytes < 64 * 1024 * 1024);
}

#[test]
fn every_reference_workload_matches_the_documented_capacity_envelope() {
    for workload in ReferenceWorkload::ALL {
        let specification = workload.specification();
        assert!(specification.channels <= 256);
        assert!(specification.routes <= 16_384);
        assert!(specification.voices <= 256);
        assert!(specification.pitch_taps + specification.spectrum_taps <= 64);
        assert!(specification.spectrum_bands <= 256);
        assert!(matches!(specification.analysis_frame_size, 0 | 2_048 | 16_384));
    }
}
