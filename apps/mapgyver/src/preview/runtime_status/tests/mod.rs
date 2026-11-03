use super::*;
#[test]
fn rates_use_actual_elapsed_time_and_reject_missing_zero_or_reset_counters() {
    let start = Instant::now();
    assert_eq!(measured_fps(start, 0, None), None);
    assert_eq!(measured_fps(start, 60, Some((start, 0))), None);
    assert_eq!(
        measured_fps(start + Duration::from_secs(2), 125, Some((start, 5))),
        Some(60.0)
    );
    assert_eq!(measured_fps(start + Duration::from_secs(1), 4, Some((start, 5))), None);
}
#[test]
fn idle_fault_recovery_is_coarse_sequenced_and_has_no_invented_metrics() {
    let mut sampler = StatusSampler::default();
    let start = Instant::now();
    let idle = sampler.sample(start, None, None).unwrap();
    assert_eq!(idle.state, VideoRuntimeState::Idle);
    assert_eq!(idle.submitted_fps, None);
    assert_eq!(idle.resources, None);
    assert!(sampler.sample(start + Duration::from_millis(999), None, None).is_none());
    let failed = sampler
        .sample(start + Duration::from_secs(1), None, Some("failed"))
        .unwrap();
    assert_eq!(failed.state, VideoRuntimeState::Faulted);
    assert_eq!(failed.sequence, 2);
    let ready = sampler.sample(start + Duration::from_secs(2), None, None).unwrap();
    assert_eq!(ready.error, None);
    assert_eq!(ready.sequence, 3);
    let view = StatusView::default();
    view.publish(ready.clone());
    assert_eq!(view.read(), ready);
    let expired = view.read_at(Instant::now() + Duration::from_secs(4));
    assert_eq!(expired.state, VideoRuntimeState::Faulted);
    assert_eq!(expired.submitted_fps, None);
    assert_eq!(expired.resources, None);
}

#[cfg(all(windows, feature = "native-video"))]
#[test]
fn project_replacement_resets_the_fps_baseline() {
    use golden_video::runtime::{NullConfig, VideoRuntime};
    let config = crate::preview::host::config::configuration(None).video;
    let mut runtime = VideoRuntime::spawn_null(config, NullConfig::default()).unwrap();
    let mut snapshot = runtime.snapshot();
    snapshot.state = RuntimeState::Running;
    snapshot.frames_submitted = 10;
    let mut sampler = StatusSampler::default();
    let start = Instant::now();
    assert_eq!(
        sampler
            .sample(start, Some((&snapshot, "GPU")), None)
            .unwrap()
            .submitted_fps,
        None
    );
    snapshot.frames_submitted = 70;
    assert_eq!(
        sampler
            .sample(start + Duration::from_secs(1), Some((&snapshot, "GPU")), None)
            .unwrap()
            .submitted_fps,
        Some(60.0)
    );
    snapshot.project = runtime.replace_project().unwrap().1;
    snapshot.frames_submitted = 80;
    assert_eq!(
        sampler
            .sample(start + Duration::from_secs(2), Some((&snapshot, "GPU")), None)
            .unwrap()
            .submitted_fps,
        None
    );
    runtime.close();
    assert!(runtime.retirement().wait(Duration::from_secs(5)));
}
