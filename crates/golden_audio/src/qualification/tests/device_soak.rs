use std::{fs, time::Duration};

use crate::{GainDb, NullBackend, SampleRate};

use super::super::{
    DeadlineMissAcceptance, ManagedDeviceSoakOptions, ReferenceWorkload, run_managed_device_soak, write_reference_wave,
};

#[test]
fn managed_soak_options_reject_offline_and_unbounded_timing_shapes() {
    assert_eq!(
        ManagedDeviceSoakOptions::default().deadline_miss_acceptance,
        DeadlineMissAcceptance::RequireZero
    );

    let zero_duration = ManagedDeviceSoakOptions {
        duration: Duration::ZERO,
        ..ManagedDeviceSoakOptions::default()
    };
    assert!(zero_duration.validate().is_err());

    let oversized_poll = ManagedDeviceSoakOptions {
        duration: Duration::from_secs(1),
        poll_interval: Duration::from_secs(2),
        ..ManagedDeviceSoakOptions::default()
    };
    assert!(oversized_poll.validate().is_err());

    let offline_workload = ManagedDeviceSoakOptions {
        duration: Duration::from_secs(1),
        poll_interval: Duration::from_millis(20),
        workload: ReferenceWorkload::ExtremeOffline,
        ..ManagedDeviceSoakOptions::default()
    };
    assert!(offline_workload.validate().is_err());
}

#[test]
fn reference_wave_is_a_bounded_stereo_pcm_fixture() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reference.wav");
    write_reference_wave(&path, Duration::from_millis(20), SampleRate::default()).unwrap();

    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(bytes.len(), 44 + 48_000 * 2 * 2 / 50);
}

#[test]
fn null_backend_soak_advances_signal_and_completes_planned_recovery() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reference.wav");
    write_reference_wave(&path, Duration::from_secs(1), SampleRate::default()).unwrap();
    let options = ManagedDeviceSoakOptions {
        duration: Duration::from_millis(700),
        poll_interval: Duration::from_millis(25),
        readiness_timeout: Duration::from_secs(2),
        recovery_interval: Some(Duration::from_millis(200)),
        deadline_miss_acceptance: DeadlineMissAcceptance::RecordOnly,
        workload: ReferenceWorkload::Medium,
        playback_gain: GainDb::new(-48.0).unwrap(),
        revision: "null-backend-test".to_owned(),
        ..ManagedDeviceSoakOptions::default()
    };

    let report = run_managed_device_soak(Box::new(NullBackend), options, &path).unwrap();

    assert!(report.passed, "{:#?}", report.failures);
    assert!(report.runtime.rendered_frames > 0);
    assert!(report.output_global_max_rms > 0.0);
    assert_eq!(report.deadline_miss_acceptance, DeadlineMissAcceptance::RecordOnly);
    assert!(report.playback_starts >= 32);
    assert!(report.attempted_recovery_cycles >= 2);
    assert_eq!(report.completed_recovery_cycles, report.attempted_recovery_cycles);
}
