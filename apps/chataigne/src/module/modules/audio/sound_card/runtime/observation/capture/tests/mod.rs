use super::*;
use std::time::Duration;

#[test]
fn module_connected_accepts_either_ready_direction() {
    use golden_audio::AudioDeviceReadiness::{Ready, Recovering};
    let mut snapshot = AudioObservationSnapshot::default();
    snapshot.device.input.enabled = true;
    snapshot.device.input.readiness = Ready;
    snapshot.device.output.enabled = true;
    snapshot.device.output.readiness = Recovering;
    assert!(ObservationHeader::from_snapshot(&snapshot).connected());
    snapshot.device.input.readiness = Recovering;
    assert!(!ObservationHeader::from_snapshot(&snapshot).connected());
    snapshot.device.output.readiness = Ready;
    assert!(ObservationHeader::from_snapshot(&snapshot).connected());
    snapshot.enabled = false;
    assert!(!ObservationHeader::from_snapshot(&snapshot).connected());
}

#[test]
fn retained_observation_capacity_failure_is_visible_and_recovers_without_actor_copying() {
    let mut engine = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .build()
        .unwrap();
    let pool = PayloadPool::new("observation-capture-test", 2, 2 * SNAPSHOT_BYTES).unwrap();
    let (capture, mut collector) = ObservationCollector::using(engine.observations(), pool.clone()).unwrap();
    let exported = capture.latest();
    let repeated = capture.latest();
    assert!(
        std::ptr::eq(&*exported, &*repeated),
        "actor reads share the captured snapshot"
    );
    drop(repeated);
    collector.refresh();
    assert!(capture.latest().values_available());
    collector.refresh();
    let full = capture.latest();
    assert_eq!(full.error, Some(ObservationCaptureError::Capacity));
    let sequence = full.sequence;
    collector.refresh();
    assert_eq!(
        capture.latest().sequence,
        sequence,
        "failed capture cannot replay stale values"
    );
    engine
        .control()
        .submit(golden_audio::AudioCommand::SetEnabled(false))
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while engine.observations().inspect(|snapshot| snapshot.enabled) {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    collector.refresh();
    let disabled = capture.latest();
    assert_eq!(disabled.error, Some(ObservationCaptureError::Capacity));
    assert!(
        !disabled.header.enabled,
        "control health must remain current under metering pressure"
    );
    assert_eq!(disabled.sequence, sequence);
    drop(disabled);
    drop((exported, full));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        collector.refresh();
        if capture.latest().values_available() {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_ne!(capture.latest().sequence, sequence);
    assert!(!capture.latest().enabled);
    drop((capture, collector));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    engine.shutdown().unwrap();
}

pub(in super::super) fn view_for_test(snapshot: AudioObservationSnapshot, sequence: u64) -> ObservationView {
    let pool = PayloadPool::new("sound-card-test-frame", 1, SNAPSHOT_BYTES).unwrap();
    let header = ObservationHeader::from_snapshot(&snapshot);
    let bytes = snapshot
        .retained_bytes_within(SNAPSHOT_BYTES - 16, SNAPSHOT_ITEMS)
        .unwrap()
        + 16;
    ObservationView {
        snapshot: pool.try_retain(snapshot, bytes).unwrap(),
        error: None,
        header,
        sequence,
        telemetry: None,
    }
}
