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
        routes: RouteView::default(),
    }
}

#[test]
fn route_capture_recovers_independently_of_full_metering_and_route_pools() {
    use super::super::routing::RouteValidationError;
    use golden_audio::{
        AudioCommand, AudioConfiguration, AudioDeviceSelection, AudioDirection, ConfigGeneration, NullBackend,
    };
    let mut engine = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(NullBackend)
        .build()
        .unwrap();
    let reader = engine.observations();
    let pool = PayloadPool::new("route-metering-test", 1, SNAPSHOT_BYTES).unwrap();
    let routes = PayloadPool::new("route-admission-test", 2, 16 * 1024 * 1024).unwrap();
    let (capture, mut collector) = ObservationCollector::using(reader.clone(), pool.clone()).unwrap();
    collector.routes = RoutePreparer::using(routes.clone());
    collector.refresh();
    let initial = capture.latest();
    let blocker = routes.try_retain((), 1).unwrap();

    let mut configuration = AudioConfiguration::empty();
    configuration.physical_outputs = ["output:0", "output:1"]
        .map(|key| golden_audio::PhysicalChannelKey::new(key).unwrap())
        .into();
    configuration.output.enabled = true;
    configuration.output.device = Some(AudioDeviceSelection::follow_system_default(
        NullBackend::backend_id(),
        AudioDirection::Output,
    ));
    configuration.validate(&golden_audio::EngineLimits::default()).unwrap();
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation: ConfigGeneration::new(1),
            config: Arc::new(configuration),
        })
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !reader.inspect(|snapshot| {
        snapshot.generation == ConfigGeneration::new(1) && snapshot.device.output.active_target.is_some()
    }) {
        assert!(std::time::Instant::now() < deadline, "Null stereo output must open");
        std::thread::sleep(Duration::from_millis(1));
    }
    collector.refresh();
    assert_eq!(capture.latest().error, Some(ObservationCaptureError::Capacity));
    assert_eq!(
        capture
            .latest()
            .routes
            .validate(reader.device_revision(), AudioDirection::Output, "output:0"),
        Err(RouteValidationError::Capacity)
    );
    drop(blocker);
    let ready = loop {
        collector.refresh();
        let view = capture.latest();
        if view
            .routes
            .validate(reader.device_revision(), AudioDirection::Output, "output:0")
            .is_ok()
        {
            break view;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "routing must recover without metering capacity"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(ready.error, Some(ObservationCaptureError::Capacity));
    assert!(
        routes.try_reserve(1).is_err(),
        "retained old and current routes both stay charged"
    );
    collector.refresh();
    assert!(
        ready.routes.same_storage(&capture.latest().routes),
        "unchanged routes reuse capacity even while full"
    );

    let ready_revision = reader.device_revision();
    engine
        .control()
        .submit(AudioCommand::ApplyConfiguration {
            generation: ConfigGeneration::new(2),
            config: Arc::new(AudioConfiguration::empty()),
        })
        .unwrap();
    while reader.device_revision() == ready_revision {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        ready
            .routes
            .validate(reader.device_revision(), AudioDirection::Output, "output:0"),
        Err(RouteValidationError::Stale)
    );
    drop(initial);
    loop {
        collector.refresh();
        if capture
            .latest()
            .routes
            .validate(reader.device_revision(), AudioDirection::Output, "output:0")
            == Err(RouteValidationError::NoDevice)
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "device removal must replace the route frame"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    drop((ready, capture, collector));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert!(routes.wait_for_idle(Duration::from_secs(3)));
    engine.shutdown().unwrap();
}
